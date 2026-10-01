/**
 * Meshes: their vertex and index buffers on the engine's device, and the vertex layout a pipeline
 * reads them through (R01 Design note 23).
 *
 * @remarks
 * Every attribute is its own tightly packed `float32` buffer, at the `@location` of its place in
 * the mesh: `position` at 0, then `MeshSpec.attributes` in insertion order, then
 * `instanceAttributes`, stepped once per instance. The buffers are made through the engine's one
 * creation path, category `other`, so their bytes and uploads are counted.
 */

import { BUFFER_USAGE } from "../gpuFlags";
import type { BufferSpec } from "../memory";
import type { BufferHandle, MeshSpec, VertexAttribute } from "../types";

/** The `GPUVertexFormat` of a float attribute of `components`. */
export function floatVertexFormat(components: number): GPUVertexFormat {
  switch (components) {
    case 1:
      return "float32";
    case 2:
      return "float32x2";
    case 3:
      return "float32x3";
    case 4:
      return "float32x4";
    default:
      throw new Error(`a float attribute has 1 to 4 components, not ${components}`);
  }
}

/** One attribute of a mesh, at its location. */
export interface MeshAttribute {
  readonly name: string;
  readonly location: number;
  readonly attribute: VertexAttribute;
  readonly instanced: boolean;
}

/** A mesh's attributes in location order: position, its attributes, its instance attributes. */
export function meshAttributes(spec: MeshSpec): ReadonlyArray<MeshAttribute> {
  const attributes: MeshAttribute[] = [
    {
      name: "position",
      location: 0,
      attribute: { data: spec.positions, size: 3 },
      instanced: false,
    },
  ];
  for (const [name, attribute] of Object.entries(spec.attributes)) {
    attributes.push({ name, location: attributes.length, attribute, instanced: false });
  }
  for (const [name, attribute] of Object.entries(spec.instanceAttributes ?? {})) {
    attributes.push({ name, location: attributes.length, attribute, instanced: true });
  }
  return attributes;
}

/** The vertex buffers a pipeline reads a mesh's attributes through, one a slot. */
export function vertexBufferLayouts(
  attributes: ReadonlyArray<MeshAttribute>,
): ReadonlyArray<GPUVertexBufferLayout> {
  return attributes.map(({ location, attribute, instanced }) => ({
    arrayStride: attribute.size * 4,
    stepMode: instanced ? "instance" : "vertex",
    attributes: [
      { shaderLocation: location, offset: 0, format: floatVertexFormat(attribute.size) },
    ],
  }));
}

/**
 * Checks a mesh's data: whole vertices and instances, attributes as long as the positions, and
 * indices inside them.
 *
 * @throws Error naming the mesh and what is wrong.
 */
export function assertMeshData(spec: MeshSpec): void {
  const vertices = spec.positions.length / 3;
  if (!Number.isInteger(vertices)) {
    throw new Error(`mesh ${spec.name}'s positions are not whole xyz triples`);
  }
  for (const [name, { data, size }] of Object.entries(spec.attributes)) {
    if (data.length !== vertices * size) {
      throw new Error(`mesh ${spec.name}'s attribute ${name} does not give each vertex ${size}`);
    }
  }
  for (const [name, { data, size }] of Object.entries(spec.instanceAttributes ?? {})) {
    if (data.length % size !== 0) {
      throw new Error(`mesh ${spec.name}'s instance attribute ${name} is not whole instances`);
    }
  }
  for (const index of spec.indices ?? []) {
    if (index >= vertices) {
      throw new Error(`mesh ${spec.name} has an index, ${index}, past its ${vertices} vertices`);
    }
  }
}

/** What a mesh needs of the engine: its one creation path for buffers. */
export interface MeshHost {
  createBuffer(spec: BufferSpec): BufferHandle;
  writeBuffer(handle: BufferHandle, offsetBytes: number, data: ArrayBufferView): void;
  gpuBufferOf(handle: BufferHandle): GPUBuffer;
}

/** Bytes rounded up to a multiple of 4, as a buffer's size and a write's length must be. */
function wholeWords(bytes: number): number {
  return Math.max(4, Math.ceil(bytes / 4) * 4);
}

/** A mesh's buffers on the device, and what a draw of it needs. */
export class MeshRecord {
  readonly spec: MeshSpec;
  readonly attributes: ReadonlyArray<MeshAttribute>;
  readonly layouts: ReadonlyArray<GPUVertexBufferLayout>;
  /** The layout's identity, part of a pipeline's key. */
  readonly layoutKey: string;
  readonly vertexCount: number;
  readonly vertexBuffers: ReadonlyArray<GPUBuffer>;
  readonly index: { readonly buffer: GPUBuffer; readonly count: number } | null;

  constructor(host: MeshHost, spec: MeshSpec) {
    assertMeshData(spec);
    this.spec = spec;
    this.attributes = meshAttributes(spec);
    this.layouts = vertexBufferLayouts(this.attributes);
    this.layoutKey = JSON.stringify([
      spec.topology,
      this.attributes.map(({ attribute, instanced }) => [attribute.size, instanced]),
    ]);
    this.vertexCount = spec.positions.length / 3;
    this.vertexBuffers = this.attributes.map(({ name, attribute }) =>
      upload(host, `${spec.name}:${name}`, BUFFER_USAGE.VERTEX, attribute.data),
    );
    this.index =
      spec.indices === null
        ? null
        : {
            buffer: upload(host, `${spec.name}:indices`, BUFFER_USAGE.INDEX, spec.indices),
            count: spec.indices.length,
          };
  }
}

/** A buffer of `usage` holding `data`, made and written through the host. */
function upload(
  host: MeshHost,
  name: string,
  usage: number,
  data: Float32Array | Uint32Array,
): GPUBuffer {
  const handle = host.createBuffer({
    name,
    bytes: wholeWords(data.byteLength),
    usage: usage | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  if (data.byteLength > 0) {
    host.writeBuffer(handle, 0, data);
  }
  return host.gpuBufferOf(handle);
}
