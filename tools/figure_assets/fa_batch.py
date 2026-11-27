"""Render a contiguous slice from the same per-frame native driver used for individual PNGs."""
import os
import time

import bpy


def render_frames(scene, directory, resolution, lo, hi, apply):
    if lo >= hi:
        return []
    expected = list(range(lo, hi))
    pending, records, errors = {}, [], []
    start, end, current = scene.frame_start, scene.frame_end, scene.frame_current
    old_path = scene.render.filepath
    old_lock = scene.render.use_lock_interface
    old_persistent = scene.render.use_persistent_data
    old_overwrite = scene.render.use_overwrite
    old_ext = scene.render.use_file_extension
    # Blender catches exceptions inside Python handlers; capture and surface them after rendering.
    def prepare(sc, *unused):
        i = sc.frame_current
        if i < lo or i >= hi or errors:
            return
        try:
            f, camera = apply(i)
            pending[i] = f, camera, time.monotonic()
        except Exception as exc:
            errors.append(f"prepare frame {i}: {exc!r}")

    def saved(sc, *unused):
        i = sc.frame_current
        sample = pending.pop(i, None)
        if sample is None:
            errors.append(f"no evaluated tape frame for output {i}")
            return
        f, camera, t = sample
        records.append((i, f, camera, time.monotonic() - t))

    try:
        scene.render.resolution_x, scene.render.resolution_y = resolution
        scene.render.resolution_percentage = 100
        scene.render.image_settings.file_format = "PNG"
        scene.render.image_settings.color_mode = "RGB"
        scene.render.image_settings.color_depth = "8"
        scene.render.filepath = os.path.join(directory, "frame_#####")
        scene.render.use_file_extension = True
        scene.render.use_overwrite = True
        scene.render.use_lock_interface = True
        scene.render.use_persistent_data = True
        scene.frame_start = lo
        scene.frame_end = hi - 1
        bpy.app.handlers.frame_change_pre.append(prepare)
        bpy.app.handlers.render_write.append(saved)
        # Resolve the first native camera before Blender takes its render context.
        scene.frame_set(lo)
        bpy.ops.render.render(animation=True)
    finally:
        if prepare in bpy.app.handlers.frame_change_pre:
            bpy.app.handlers.frame_change_pre.remove(prepare)
        if saved in bpy.app.handlers.render_write:
            bpy.app.handlers.render_write.remove(saved)
        scene.frame_start, scene.frame_end = start, end
        scene.render.filepath = old_path
        scene.render.use_persistent_data = old_persistent
        scene.render.use_lock_interface = old_lock
        scene.render.use_overwrite = old_overwrite
        scene.render.use_file_extension = old_ext
        scene.frame_set(current)
    if errors:
        raise ValueError("; ".join(errors))
    if [i for i, _, _, _ in records] != expected:
        raise ValueError(f"animation wrote unexpected frames {[r[0] for r in records]} for [{lo}, {hi})")
    for i in expected:
        if not os.path.isfile(os.path.join(directory, f"frame_{i:05d}.png")):
            raise ValueError(f"missing output PNG for frame {i}")
    return records
