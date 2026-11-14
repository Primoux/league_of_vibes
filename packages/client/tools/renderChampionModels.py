"""Contrôle PBR hors WebGL de la géométrie réelle exportée par exportChampionModels.ts.

Blender 4.3+, sans extension. Rendu CPU reproductible, sans contour GPU ni bloom.
Usage : blender -b -t 4 -P packages/client/tools/renderChampionModels.py -- /tmp/models.json /tmp/renders [champion ...]
"""
import json
from pathlib import Path
import sys

import bpy
from mathutils import Vector

args = sys.argv[sys.argv.index('--') + 1:]
models = json.loads(Path(args[0]).read_text())
output = Path(args[1])
output.mkdir(parents=True, exist_ok=True)
if args[2:]:
    models = [m for m in models if m['id'] in args[2:]]

bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
scene = bpy.context.scene
scene.render.engine = 'CYCLES'
scene.cycles.device = 'CPU'
scene.cycles.samples = 48
scene.cycles.use_denoising = False  # Fonctionne aussi avec Blender compilé sans OIDN.
scene.render.threads_mode = 'FIXED'
scene.render.threads = 4
scene.render.resolution_x = 400
scene.render.resolution_y = 520
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = 'PNG'
scene.view_settings.view_transform = 'AgX'
scene.world.use_nodes = True
background = scene.world.node_tree.nodes['Background']
background.inputs[0].default_value = (.12, .16, .24, 1)
background.inputs[1].default_value = .35

bpy.ops.mesh.primitive_plane_add(size=200)
floor = bpy.context.object
floor.location.z = -.065
material = bpy.data.materials.new('Sol')
material.diffuse_color = (.024, .036, .055, 1)
floor.data.materials.append(material)

lights = [((3, -4, 6), 650, (1, .84, .65), 4),
          ((-3, -1, 3), 400, (.35, .6, 1), 3),
          ((0, 4, 5), 900, (.6, .75, 1), 3)]
for location, power, color, size in lights:
    bpy.ops.object.light_add(type='AREA', location=location)
    light = bpy.context.object
    light.data.energy, light.data.color, light.data.size = power, color, size
    light.data.shape = 'DISK'
    light.rotation_euler = (Vector((0, 0, 1)) - light.location).to_track_quat('-Z', 'Y').to_euler()

bpy.ops.object.camera_add(location=(6, -3.8, 3.5))
camera = bpy.context.object
camera.data.type, camera.data.ortho_scale = 'ORTHO', 3.15
camera.rotation_euler = (Vector((0, 0, 1.12)) - camera.location).to_track_quat('-Z', 'Y').to_euler()
scene.camera = camera

for model in models:
    objects = []
    factor = 2 / model['height']
    for index, source in enumerate(model['meshes']):
        positions = source['positions']
        vertices = [(positions[i] * factor, -positions[i + 2] * factor, positions[i + 1] * factor)
                    for i in range(0, len(positions), 3)]
        name = f"{model['id']}-{index}"
        mesh = bpy.data.meshes.new(name)
        mesh.from_pydata(vertices, [], [(i, i + 1, i + 2) for i in range(0, len(vertices), 3)])
        mesh.update()
        obj = bpy.data.objects.new(name, mesh)
        scene.collection.objects.link(obj)
        objects.append(obj)
        colors = mesh.color_attributes.new(name='Palette', type='FLOAT_COLOR', domain='POINT')
        colors.data.foreach_set('color', source['colors'])
        if source['smooth']:
            for polygon in mesh.polygons:
                polygon.use_smooth = True
            normals = source['normals']
            mesh.normals_split_custom_set([(normals[i], -normals[i + 2], normals[i + 1])
                                           for i in range(0, len(normals), 3)])
        material = bpy.data.materials.new(name)
        material.use_nodes = True
        nodes, links = material.node_tree.nodes, material.node_tree.links
        shader = nodes.get('Principled BSDF')
        shader.inputs['Metallic'].default_value = source['metalness']
        shader.inputs['Roughness'].default_value = source['roughness']
        color = nodes.new('ShaderNodeVertexColor')
        color.layer_name = 'Palette'
        links.new(color.outputs['Color'], shader.inputs['Base Color'])
        if source['emission']:
            links.new(color.outputs['Color'], shader.inputs['Emission Color'])
            shader.inputs['Emission Strength'].default_value = .9
        mesh.materials.append(material)
    scene.render.filepath = str(output / f"{model['id']}.png")
    bpy.ops.render.render(write_still=True)
    print('MODEL_RENDERED', model['id'], flush=True)
    for obj in objects:
        mesh = obj.data
        bpy.data.objects.remove(obj, do_unlink=True)
        bpy.data.meshes.remove(mesh)
