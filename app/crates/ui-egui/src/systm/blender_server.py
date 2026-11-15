# SPDX-License-Identifier: GPL-2.0-or-later
# SYSTM Cut's 3D page talks to Blender through this script: Blender runs in the background as its
# own program (`blender -b --python blender_server.py -- <port>`), and the page sends it one JSON
# request per line over a local socket. This file uses Blender's Python API, so it carries
# Blender's licence; it is a separate program from SYSTM Cut.
import json, math, socket, sys, traceback

import bpy
import numpy as np
from mathutils import Euler, Vector

PORT = int(sys.argv[sys.argv.index("--") + 1])


def scene_state():
    sc = bpy.context.scene
    act = bpy.context.view_layer.objects.active
    objs = []
    for o in sc.objects:
        mat = o.active_material
        col = None
        if mat and mat.use_nodes:
            b = mat.node_tree.nodes.get("Principled BSDF")
            if b:
                col = [round(float(v), 4) for v in b.inputs["Base Color"].default_value[:3]]
        objs.append({
            "name": o.name, "type": o.type, "selected": o.select_get(), "active": o == act, "visible": not o.hide_viewport and not o.hide_render,
            "location": [round(v, 4) for v in o.location], "rotation": [round(math.degrees(v), 3) for v in o.rotation_euler], "scale": [round(v, 4) for v in o.scale],
            "color": col, "modifiers": [m.type for m in o.modifiers], "parent": o.parent.name if o.parent else None,
        })
    return {"objects": objs, "frame": sc.frame_current, "frame_start": sc.frame_start, "frame_end": sc.frame_end, "fps": sc.render.fps, "file": bpy.data.filepath,
            "resolution": [sc.render.resolution_x, sc.render.resolution_y], "camera": sc.camera.name if sc.camera else None}


def view_camera(yaw, pitch, dist, target):
    """A camera of our own (not the scene's), orbiting `target`: the page's viewport."""
    cam = bpy.data.objects.get("__systm_view")
    if cam is None:
        cam = bpy.data.objects.new("__systm_view", bpy.data.cameras.new("__systm_view"))
        cam.data.lens = 50
        cam.data.clip_start, cam.data.clip_end = 0.01, 5000
    t = Vector(target)
    d = Vector((math.cos(pitch) * math.sin(yaw), -math.cos(pitch) * math.cos(yaw), math.sin(pitch)))
    cam.location = t + d * dist
    cam.rotation_euler = (t - cam.location).to_track_quat("-Z", "Y").to_euler()
    return cam


def render_to(path, raw, cam, w, h, engine, frame=None):
    sc = bpy.context.scene
    keep = (sc.camera, sc.render.resolution_x, sc.render.resolution_y, sc.render.resolution_percentage, sc.render.engine, sc.render.filepath, sc.render.image_settings.file_format, sc.render.film_transparent)
    linked = False
    try:
        if cam is not None and cam.name not in sc.collection.objects:
            sc.collection.objects.link(cam)
            linked = True
        if cam is not None:
            sc.camera = cam
        if w and h:
            sc.render.resolution_x, sc.render.resolution_y, sc.render.resolution_percentage = int(w), int(h), 100
        sc.render.engine = engine
        if engine == "BLENDER_WORKBENCH":
            sh = sc.display.shading
            sh.light, sh.color_type, sh.show_cavity, sh.show_shadows = "STUDIO", "MATERIAL", True, False
            sc.display.render_aa = "FXAA"
        if frame is not None:
            sc.frame_set(int(frame))
        sc.render.image_settings.file_format = "PNG"
        sc.render.filepath = path
        bpy.ops.render.render(write_still=True)
        if raw:
            # straight RGBA8, top row first, behind a 8-byte size header: the page reads it without a decoder
            img = bpy.data.images.load(path, check_existing=False)
            iw, ih = img.size
            px = np.empty(iw * ih * 4, dtype=np.float32)
            img.pixels.foreach_get(px)
            bpy.data.images.remove(img)
            out = (np.clip(px.reshape(ih, iw, 4)[::-1], 0, 1) ** (1 / 2.2 if False else 1) * 255 + 0.5).astype(np.uint8)
            with open(raw, "wb") as f:
                f.write(np.array([iw, ih], dtype="<u4").tobytes())
                f.write(out.tobytes())
    finally:
        sc.camera, sc.render.resolution_x, sc.render.resolution_y, sc.render.resolution_percentage, sc.render.engine, sc.render.filepath, sc.render.image_settings.file_format, sc.render.film_transparent = keep
        if linked:
            sc.collection.objects.unlink(cam)


def pick(yaw, pitch, dist, target, x, y, aspect):
    """The object under a point of the viewport (x, y in 0..1 from the top left)."""
    cam = view_camera(yaw, pitch, dist, target)
    sensor = cam.data.sensor_width
    half_w = sensor / 2 / cam.data.lens
    half_h = half_w / aspect
    local = Vector(((x * 2 - 1) * half_w, (1 - y * 2) * half_h, -1.0))
    m = cam.matrix_basis
    direction = (m.to_3x3() @ local).normalized()
    hit, _, _, _, obj, _ = bpy.context.scene.ray_cast(bpy.context.evaluated_depsgraph_get(), cam.location, direction)
    return obj.name if hit and obj else None


def select(name, extend=False):
    if not extend:
        for o in bpy.context.scene.objects:
            o.select_set(False)
    o = bpy.context.scene.objects.get(name) if name else None
    if o:
        o.select_set(True)
        bpy.context.view_layer.objects.active = o
    elif not extend:
        bpy.context.view_layer.objects.active = None


def handle(req):
    op = req.get("op")
    if op == "scene":
        return scene_state()
    if op == "exec":
        # anything Blender can do: the code runs with bpy, the context and the helpers in scope
        env = {"bpy": bpy, "math": math, "Vector": Vector, "Euler": Euler, "C": bpy.context, "D": bpy.data, "result": None}
        exec(req["code"], env)
        return {"result": env.get("result"), "scene": scene_state()}
    if op == "view":
        cam = view_camera(req["yaw"], req["pitch"], req["dist"], req["target"])
        render_to(req["path"], req.get("raw"), cam, req["w"], req["h"], "BLENDER_EEVEE" if req.get("rendered") else "BLENDER_WORKBENCH")
        return {"path": req["path"]}
    if op == "render":
        # the scene's own camera, at the scene's resolution: the real picture
        if bpy.context.scene.camera is None:
            raise RuntimeError("the scene has no camera: add one (Add > Camera)")
        render_to(req["path"], None, None, req.get("w"), req.get("h"), req.get("engine", "BLENDER_EEVEE"), req.get("frame"))
        return {"path": req["path"]}
    if op == "pick":
        name = pick(req["yaw"], req["pitch"], req["dist"], req["target"], req["x"], req["y"], req["aspect"])
        select(name, req.get("extend", False))
        return {"picked": name, "scene": scene_state()}
    if op == "select":
        select(req.get("name"), req.get("extend", False))
        return scene_state()
    raise RuntimeError("unknown op %r" % op)


srv = socket.socket()
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", PORT))
srv.listen(1)
print("systm-3d ready", PORT, flush=True)
while True:
    conn, _ = srv.accept()
    f = conn.makefile("rw", encoding="utf-8")
    for line in f:
        try:
            req = json.loads(line)
            if req.get("op") == "quit":
                f.write(json.dumps({"ok": True}) + "\n"); f.flush()
                sys.exit(0)
            out = {"ok": True, "data": handle(req)}
        except SystemExit:
            raise
        except Exception as e:
            out = {"ok": False, "error": "%s: %s" % (type(e).__name__, e), "trace": traceback.format_exc()[-600:]}
        f.write(json.dumps(out) + "\n"); f.flush()
    conn.close()
