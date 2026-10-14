/* C ABI of against_rome_data (ABI version 1). Ownership: see README.md. */
#ifndef AGAINST_ROME_DATA_H
#define AGAINST_ROME_DATA_H
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum {
    AR_OK = 0, AR_NULL_ARGUMENT = 1, AR_BUFFER_TOO_SMALL = 2, AR_BAD_STRUCT_SIZE = 3,
    AR_INVALID_SLOT = 4, AR_INACTIVE_SLOT = 5, AR_PANIC = 6,
    AR_NONFINITE_POSITION = 7, AR_RESULT_OVERFLOW = 8,
    AR_PFIL_HEADER_TRUNCATED = 100, AR_PFIL_BAD_MAGIC = 101, AR_DECLARED_SIZE = 102,
    AR_LZSS_TRUNCATED = 103, AR_LZSS_OVERSHOOT = 104, AR_LZSS_TRAILING_INPUT = 105,
    AR_PAYLOAD_LENGTH = 110, AR_BAD_VERSION = 111, AR_BAD_CAPACITY = 112
};

typedef struct ArLightPool ArLightPool; /* opaque */

typedef struct ArLightSnapshot {
    uint32_t struct_size; /* caller sets to ar_light_snapshot_struct_size() */
    uint32_t slot;
    uint32_t active_raw;
    float position[3];
    float color[3];
    float effective_radius;
    float baseline_radius;
    float type_parameter;
    uint32_t age_raw;     /* reinterpret as int32 for the signed reading */
    uint32_t special_fx_raw;
    uint16_t type_raw;
    int16_t owner;
    int16_t parent;
    int16_t definition;
    uint8_t raw_fields[56]; /* authoritative saved bytes */
} ArLightSnapshot;

uint32_t ar_light_abi_version(void);
uint32_t ar_light_snapshot_struct_size(void);
const char* ar_light_error_message(int32_t code); /* static, do not free */

int32_t ar_light_pool_open(const uint8_t* data, size_t len, ArLightPool** out);
void ar_light_pool_free(ArLightPool* pool);
int32_t ar_light_pool_active_count(const ArLightPool* pool, uint32_t* out);
int32_t ar_light_pool_snapshot(const ArLightPool* pool, uint32_t slot, ArLightSnapshot* out);
int32_t ar_light_pool_snapshot_any(const ArLightPool* pool, uint32_t slot, ArLightSnapshot* out);
int32_t ar_light_pool_copy_payload(const ArLightPool* pool, uint8_t* buf, size_t cap, size_t* out_len);
int32_t ar_light_pool_copy_pfil_header(const ArLightPool* pool, uint8_t* buf, size_t cap, size_t* out_len);


/* ---- Saved-light projection (appended; ABI version stays 1, layouts above unchanged) ----
 * Direct float projection of saved XYZ: delta_fixed == 0 keeps the saved bits (incl. -0.0);
 * otherwise (double)saved + (double)delta / 65536.0 -> float32, must be finite.
 * All other 44 bytes (RGB, radii, active, type, age, owner/parent/definition) are preserved.
 * NOT valid for particle-linked lights (those need active child fixed + delta, then SAR16). */
typedef struct ArLightProjectionRequest {
    uint32_t struct_size; /* ar_light_projection_request_struct_size() */
    uint32_t slot;
    uint8_t target_guid[16]; /* host identity, carried through; (guid, slot) is never deduplicated */
    int32_t delta_fixed[3];  /* signed 16.16 X, Y, Z */
} ArLightProjectionRequest;

typedef struct ArProjectedLight {
    uint32_t struct_size; /* caller sets to ar_projected_light_struct_size() on every element */
    uint32_t request_index;
    uint8_t target_guid[16];
    uint32_t slot;
    uint32_t active_raw;
    float position[3];
    float color[3];
    float effective_radius;
    float baseline_radius;
    float type_parameter;
    uint32_t age_raw;
    uint32_t special_fx_raw;
    uint16_t type_raw;
    int16_t owner;
    int16_t parent;
    int16_t definition;
    uint8_t raw_fields[56]; /* authoritative projected bytes */
} ArProjectedLight;

uint32_t ar_light_projection_request_struct_size(void);
uint32_t ar_projected_light_struct_size(void);

/* Atomic: whole batch validated first. count==0 -> OK (pointers may be NULL).
 * out_capacity < count -> AR_BUFFER_TOO_SMALL, *out_count = count, nothing written.
 * On any error out[] is untouched, *out_count = 0 and *failed_index (nullable) = offending index
 * (0xFFFFFFFF if not request-specific). Errors: INVALID_SLOT, INACTIVE_SLOT, NONFINITE_POSITION
 * (non-finite saved coordinate on an axis with non-zero delta), RESULT_OVERFLOW, BAD_STRUCT_SIZE.
 * Pool is never modified. out must not overlap requests. */
int32_t ar_light_pool_project_batch(const ArLightPool* pool, const ArLightProjectionRequest* requests,
                                    size_t count, ArProjectedLight* out, size_t out_capacity,
                                    size_t* out_count, uint32_t* failed_index);

/* ---- Particle-linked saved-light projection (appended; ABI version stays 1) ----
 * Mirrors NativeParticlePool.TranslateBatchFixed for the light side only. `records` is a contiguous
 * buffer of record_count saved emitter records, 2252 bytes each (never modified). Per request the child
 * (record_index, child_ordinal) must link (i16 at record+12+35*ordinal+33) to light_slot, the record's
 * active u16 at offset 0 must be non-zero, and the Rust pool light at light_slot must be active.
 *  - child active byte (record+12+35*ordinal+0) != 0: fixed XYZ i32 at +1/+5/+9 each get delta_fixed
 *    via checked add (AR_FIXED_OVERFLOW on i32 overflow), then arithmetic >> 16, then float32 light
 *    position. A zero delta still derives the position this way (it does NOT keep the saved light bits).
 *  - child inactive: saved light XYZ bits kept verbatim; fixed_position reports the saved values.
 * All other light bytes preserved. Atomic like ar_light_pool_project_batch (validate all, then write).
 * Errors: BAD_STRUCT_SIZE, INVALID_RECORD_INDEX, INACTIVE_EMITTER, INVALID_CHILD, INVALID_SLOT,
 * LIGHT_LINK_MISMATCH, INACTIVE_SLOT, FIXED_OVERFLOW, BAD_RECORDS_LENGTH, NULL_ARGUMENT, BUFFER_TOO_SMALL.
 * Not a claim about particle sprite/frame/blend behaviour. */
enum { AR_EMITTER_RECORD_SIZE = 2252, AR_PARTICLE_CHILD_COUNT = 64,
       AR_INVALID_RECORD_INDEX = 9, AR_INACTIVE_EMITTER = 10, AR_INVALID_CHILD = 11,
       AR_LIGHT_LINK_MISMATCH = 12, AR_FIXED_OVERFLOW = 13, AR_BAD_RECORDS_LENGTH = 14 };

typedef struct ArParticleLightRequest {
    uint32_t struct_size; /* ar_particle_light_request_struct_size() == 44 */
    uint32_t record_index;
    uint32_t child_ordinal;
    uint32_t light_slot;
    uint8_t target_guid[16]; /* host identity, never deduplicated */
    int32_t delta_fixed[3];  /* signed 16.16 X, Y, Z */
} ArParticleLightRequest;

typedef struct ArParticleProjectedLight {
    uint32_t struct_size; /* ar_particle_projected_light_struct_size() == 160, set on every element; raw_fields@84, child_ordinal@140, child_active_raw@144, fixed_position@148 */
    uint32_t request_index;
    uint8_t target_guid[16];
    uint32_t slot;
    uint32_t active_raw;
    float position[3];
    float color[3];
    float effective_radius;
    float baseline_radius;
    float type_parameter;
    uint32_t age_raw;
    uint32_t special_fx_raw;
    uint16_t type_raw;
    int16_t owner;
    int16_t parent;
    int16_t definition;
    uint8_t raw_fields[56];
    uint32_t child_ordinal;
    uint32_t child_active_raw;
    int32_t fixed_position[3]; /* active: fixed + delta; inactive: saved, unchanged */
} ArParticleProjectedLight;

uint32_t ar_particle_light_request_struct_size(void);
uint32_t ar_particle_projected_light_struct_size(void);

int32_t ar_light_pool_project_particle_batch(const ArLightPool* pool, const uint8_t* records,
                                             size_t record_count, const ArParticleLightRequest* requests,
                                             size_t count, ArParticleProjectedLight* out,
                                             size_t out_capacity, size_t* out_count, uint32_t* failed_index);


/* ---- Immutable saved particle.dat pool (additive; existing ABI1 unchanged) ----
 * All pointers must refer to valid caller-owned storage of the stated size.
 * Open copies input; failure sets *out=NULL. Free exactly once; null is a no-op.
 * No runtime simulation, sprite/frame/blend inference or semantic rejection of unused slots.
 * Error outputs are untouched except open's handle and copy_*'s required out_len.
 * All buffers and structs must be disjoint. Calls never return interior pointers.
 */
enum { AR_PARTICLE_BAD_CHILDREN = 113, AR_PARTICLE_BAD_RENDERABLE_COUNT = 114,
       AR_PARTICLE_IO_ERROR = 115 };
typedef struct ArParticlePool ArParticlePool;
typedef struct ArParticlePoolSnapshot {
    uint32_t struct_size; /* ar_particle_pool_snapshot_struct_size(): 24 */
    uint32_t slot;
    uint32_t active_raw; /* saved u16, zero-extended; any nonzero is active */
    int32_t count_raw;
    int16_t owner;
    uint16_t reserved; /* output zero */
    int32_t age_raw;
} ArParticlePoolSnapshot;
uint32_t ar_particle_pool_abi_version(void); /* 1 */
uint32_t ar_particle_pool_snapshot_struct_size(void);
size_t ar_particle_pool_record_size(void); /* 2252 */
size_t ar_particle_pool_state_size(void); /* 16 */
size_t ar_particle_pool_child_size(void); /* 35 */
size_t ar_particle_pool_payload_size(void); /* 2322444 */
const char* ar_particle_pool_error_message(int32_t code); /* static, never free */
int32_t ar_particle_pool_open(const uint8_t* data, size_t len, ArParticlePool** out);
void ar_particle_pool_free(ArParticlePool* pool);
int32_t ar_particle_pool_active_count(const ArParticlePool* pool, uint32_t* out);
/* Three outputs are atomic; metadata struct_size must be initialized.
 * record = exact saved 2252 bytes; state = four original little-endian i32 words.
 * snapshot requires active emitter; snapshot_any accepts inactive slots. */
int32_t ar_particle_pool_snapshot(const ArParticlePool* pool, uint32_t slot,
    ArParticlePoolSnapshot* out, uint8_t* record, size_t record_cap, uint8_t* state, size_t state_cap);
int32_t ar_particle_pool_snapshot_any(const ArParticlePool* pool, uint32_t slot,
    ArParticlePoolSnapshot* out, uint8_t* record, size_t record_cap, uint8_t* state, size_t state_cap);
/* No count/active gate: slot <1024, ordinal <64, cap >=35. */
int32_t ar_particle_pool_child_any(const ArParticlePool* pool, uint32_t slot, uint32_t ordinal,
    uint8_t* buf, size_t cap);
int32_t ar_particle_pool_validate_renderable_count(const ArParticlePool* pool, uint32_t slot, uint32_t* out);
/* Two-pass copies: *out_len gets required size; null/short buffer => BUFFER_TOO_SMALL.
 * Raw input has zero PFIL header bytes. SHA256 identifies original input, including PFIL wrapper. */
int32_t ar_particle_pool_copy_payload(const ArParticlePool* pool, uint8_t* buf, size_t cap, size_t* out_len);
int32_t ar_particle_pool_copy_pfil_header(const ArParticlePool* pool, uint8_t* buf, size_t cap, size_t* out_len);
int32_t ar_particle_pool_copy_input_sha256(const ArParticlePool* pool, uint8_t* buf, size_t cap, size_t* out_len);

/* ---- Explicit indexed spans / editor ALR still-frame scanlines ----
 * Lengths are element counts. Nonempty input storage must be readable; output writable,
 * u32 storage aligned. Output may overlap input: full result is staged before writing.
 * Error leaves output unchanged; success preserves the unused output capacity tail.
 * No retained pointers, animation/frame selection, native blend/endpoint dispatch.
 */
enum { AR_PIXEL_INVALID_MODE = 200, AR_PIXEL_INVALID_INDEX = 201,
       AR_ALR_INVALID_SCANLINE = 210 };
uint32_t ar_particle_pixel_abi_version(void); /* 1 */
/* mode 0=replacement, 1=half-average, 2=raw four-lane palette-alpha/256;
 * skip_zero 0=always write, 1=skip zero. Mode2 preserves raw high-byte semantics,
 * not editor ARGB conversion. Caller selects the verified route explicitly.
 * Empty row accepts null inputs/output. */
int32_t ar_particle_pixel_row(const uint8_t* indices, size_t count,
    const uint32_t* palette, size_t palette_len, const uint32_t* destination,
    uint32_t mode, uint32_t skip_zero, uint32_t* output, size_t output_capacity);
uint32_t ar_alr_scanline_abi_version(void); /* 1 */
/* width/height <=2047, row_count=height+1, palette_len=1..511.
 * Null only for empty data/output. Converts supplied 00BBGGRR colors to editor ARGB.
 * Writes width*height pixels, transparent regions zero. */
int32_t ar_alr_decode_argb(uint32_t width, uint32_t height,
    const uint32_t* rows, size_t row_count, const uint8_t* data, size_t data_len,
    const uint32_t* palette, size_t palette_len, uint32_t* output, size_t output_capacity);

/* ---- Caller-supplied BASE particle/object definition file images ----
 * No filesystem reads, overlays, source selection or asset residency inference.
 * All inputs copied into owned catalogs. Nonempty inputs readable for length;
 * out writable and disjoint. Open clears *out on failure; free exactly once.
 * Query requires live handle and output.struct_size=44, output disjoint from handle.
 * Query errors leave all output fields unchanged. Synchronize queries with free.
 */
typedef struct ArParticleDefinitions ArParticleDefinitions;
typedef struct ArParticleDefinition {
    uint32_t struct_size;
    int32_t owner_type, parameter_id, geometry_id, grafik_id, grafik_object, alr_id;
    int32_t lifetime, type_divisor, velocity_branch_raw, skip_first_raw;
} ArParticleDefinition;
uint32_t ar_particle_definition_abi_version(void); /* 1 */
uint32_t ar_particle_definition_struct_size(void); /* 44 */
/* codes: 220 parameter, 221 geometry, 222 grafik, 223 object parser rejection;
 * 224 missing/inactive chain, 225 explicitly no primary grafik object.
 * Other codes follow the existing shared ABI error codes. */
int32_t ar_particle_definition_open(const uint8_t* par, size_t par_len,
    const uint8_t* geo, size_t geo_len, const uint8_t* gfx, size_t gfx_len,
    const uint8_t* obj, size_t obj_len, ArParticleDefinitions** out);
void ar_particle_definition_free(ArParticleDefinitions* handle);
int32_t ar_particle_definition_query(const ArParticleDefinitions* handle,
    int32_t owner_type, ArParticleDefinition* output);
/* Checked loose-file snapshot: absolute UTF-8 root; DefaultPath/dau/d00..d09.
 * Includes verified selected-field initialization. Code226 = source/encoding/load failure.
 * Failure clears *out. Does not watch files or establish native VFS/resident asset state. */
int32_t ar_particle_definition_load_utf8(const uint8_t* root, size_t len, ArParticleDefinitions** out);
/* Native file read count (including clengine); caller-buffer open reports zero. */
int32_t ar_particle_definition_source_count(const ArParticleDefinitions* handle, uint32_t* out);
/* Lifetime-only frame query. Caller supplies verified resident ALR layout.
 * Success writes *out; errors leave it unchanged. Synchronize with free.
 * 240 layout, 241 velocity unverified, 242 direction unverified,
 * 243 zero divisor, 244 signed division overflow, 245 negative index,
 * 246 frame outside payload, 247 unsupported asset route, 248 ALR ID mismatch.
 * Chain errors use 224/225. ALR identity does not establish asset residency. */
int32_t ar_particle_definition_lifetime_frame(const ArParticleDefinitions* handle,
    int32_t owner_type, int32_t child_lifetime, int32_t alr_id,
    int32_t columns, int32_t directions, uint32_t frame_count, uint32_t* out);
/* ABI1, size40. frame=UINT32_MAX when status!=0. Native signed SAR16 coordinates.
 * Count-bounded active children, skip-first parameter applied; not native visibility.
 * Query errors leave output/count unchanged; per-child unavailable status is successful query.
 * Exact record2252/state16 lengths. Null output permitted only for an empty result.
 * Inputs/outputs disjoint, synchronize with catalog free. */
typedef struct ArParticleFrameCandidate {
    uint32_t ordinal;
    int32_t position_native[3], velocity_high[3], lifetime_raw;
    uint32_t frame;
    int32_t status;
} ArParticleFrameCandidate;
uint32_t ar_particle_frame_candidate_struct_size(void); /* 40 */
/* Editor translation: checked fixed addition to ALL active children before selection.
 * Error13 = fixed overflow (including out-of-count/skip-first child). Baseline unchanged.
 * Same buffer/lifetime/atomic-output preconditions as saved_frames below. */
int32_t ar_particle_definition_saved_frames_translated(const ArParticleDefinitions* handle,
    int32_t owner_type, uint32_t slot, const uint8_t* record, size_t record_len,
    const uint8_t* state, size_t state_len, int32_t alr_id, int32_t columns,
    int32_t directions, uint32_t frame_count, int32_t dx, int32_t dy, int32_t dz,
    ArParticleFrameCandidate* output, size_t capacity, uint32_t* count);
int32_t ar_particle_definition_saved_frames(const ArParticleDefinitions* handle,
    int32_t owner_type, uint32_t slot, const uint8_t* record, size_t record_len,
    const uint8_t* state, size_t state_len, int32_t alr_id, int32_t columns,
    int32_t directions, uint32_t frame_count, ArParticleFrameCandidate* output,
    size_t capacity, uint32_t* count);

/* Detached objects record79 + decoded gfxtype payload. Output size24; buffers
 * readable/writable and disjoint. Rejections leave output unchanged.
 * 249 missing primary context,250 invalid option,251 UID mismatch,252 saved data
 * unresolved. leaf_address0 means unknown/no leaf. Primary type provenance is
 * the caller's responsibility; this is not a native visibility/draw contract. */
typedef struct ArSavedObjectMode {
    uint32_t struct_size, object_uid, gfx_slot;
    int32_t gfx_mode, resolved_mode;
    uint32_t leaf_address;
} ArSavedObjectMode;
uint32_t ar_particle_saved_object_mode_struct_size(void);
/* Owned effective chain -> primary grafik type mode. Output unchanged on error;
 * 224 chain unavailable,225 no primary,249 mode not established. Synchronize/free. */
int32_t ar_particle_definition_primary_mode(const ArParticleDefinitions* handle,
    int32_t owner_type, int32_t* output);
int32_t ar_particle_saved_object_mode(const uint8_t* object, size_t object_len,
    const uint8_t* gfx, size_t gfx_len, uint32_t expected_uid, int32_t primary_mode,
    uint32_t has_primary, ArSavedObjectMode* output);
#ifdef __cplusplus
}
#endif
#endif
