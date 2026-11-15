"""SYSTM Cut plugin SDK (Python). A plugin started from Workspace > Scripts drives the editor with it.

    from systm_cut import app
    app.call("ui.click", {"id": "page.color"})        # anything a person or an agent can do
    app.execute("markers.add", {...})                 # an engine command
    print("done")                                     # the last line printed is shown in the status bar
"""
import json, os, socket


class App:
    def __init__(self):
        self.port = int(os.environ["SYSTM_CUT_PORT"])
        with open(os.environ["SYSTM_CUT_TOKEN_FILE"]) as f:
            self.token = f.read().strip()
        self.sock = socket.create_connection(("127.0.0.1", self.port))
        self.file = self.sock.makefile("rw")
        self.n = 0

    def call(self, method, params=None):
        """Call a control method; returns its result, raises RuntimeError on an error."""
        self.n += 1
        p = dict(params or {})
        p.setdefault("_agent", os.environ.get("SYSTM_AGENT_ID", "script"))
        self.file.write(json.dumps({"id": self.n, "token": self.token, "method": method, "params": p}) + "\n")
        self.file.flush()
        r = json.loads(self.file.readline())
        if not r.get("ok"):
            raise RuntimeError(r.get("error", "failed"))
        return r.get("result")

    def execute(self, command, params=None):
        """Run an engine command (see app.call("engine.commands"))."""
        return self.call("engine.execute", {"command": command, **(params or {})})


app = App() if "SYSTM_CUT_PORT" in os.environ else None
