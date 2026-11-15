"""Speech to text for the chat's microphone. Prints the text; exits 1 with a reason on stderr.
Keys come from ~/.env.local (ELEVENLABS_API_KEY, then GEMINI_API_KEY) and are never printed."""
import base64, json, os, subprocess, sys, tempfile

path = sys.argv[1]
env = {}
try:
    for line in open(os.path.expanduser("~/.env.local"), encoding="utf-8"):
        if "=" in line and not line.lstrip().startswith("#"):
            k, v = line.split("=", 1)
            env[k.strip().removeprefix("export ").strip()] = v.strip().strip("\"'")
except OSError:
    pass
audio = open(path, "rb").read()
errors = []


def curl(args, data=None):
    """curl does the request (the system's certificates, unlike this Python's); the key goes in by stdin config, not on the command line."""
    r = subprocess.run(["curl", "-sS", "--max-time", "90", "-K", "-"] + args, input=data, capture_output=True)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.decode(errors="replace").strip()[:120] or "curl failed")
    return json.loads(r.stdout)


def eleven():
    key = env.get("ELEVENLABS_API_KEY")
    if not key:
        raise RuntimeError("no key")
    cfg = f'header = "xi-api-key: {key}"\n'.encode()
    r = curl(["-X", "POST", "https://api.elevenlabs.io/v1/speech-to-text", "-F", "model_id=scribe_v1", "-F", f"file=@{path}"], cfg)
    if "text" not in r:
        raise RuntimeError(str(r.get("detail", {}).get("status", "refused") if isinstance(r.get("detail"), dict) else "refused"))
    return r["text"]


def gemini():
    key = env.get("GEMINI_API_KEY")
    if not key:
        raise RuntimeError("no key")
    body = {"contents": [{"parts": [{"inline_data": {"mime_type": "audio/wav", "data": base64.b64encode(audio).decode()}}, {"text": "Transcribe this speech exactly, in the language spoken (Hebrew stays Hebrew). Output only the transcript, no notes."}]}]}
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(body, f)
    try:
        cfg = f'header = "x-goog-api-key: {key}"\nheader = "Content-Type: application/json"\n'.encode()
        r = curl(["-X", "POST", "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-pro-preview:generateContent", "--data-binary", f"@{f.name}"], cfg)
    finally:
        os.unlink(f.name)
    if "candidates" not in r:
        raise RuntimeError(str(r.get("error", {}).get("status", "refused")))
    return "".join(p.get("text", "") for p in r["candidates"][0]["content"]["parts"])


for name, fn in (("ElevenLabs", eleven), ("Gemini", gemini)):
    try:
        text = fn().strip()
        if text:
            print(text)
            sys.exit(0)
        errors.append(f"{name}: empty")
    except Exception as e:  # the next service is the fallback
        errors.append(f"{name}: {e}")
print("; ".join(errors), file=sys.stderr)
sys.exit(1)
