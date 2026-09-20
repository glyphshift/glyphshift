#pragma once

#include <cstddef>
#include <cstdint>

// C/C++ mirror of crates/adapters/platform/native-abi.
// The Rust loader remains authoritative; static assertions catch layout drift.
struct FixedUtf8 {
    uint16_t len;
    uint8_t bytes[96];
};

struct Descriptor {
    uint32_t size;
    FixedUtf8 id;
    uint16_t major;
    uint16_t minor;
    uint16_t patch;
    uint16_t abi_major;
    uint16_t abi_minor;
    uint32_t apply_model;
    uint32_t placement;
    uint64_t features;
    uint32_t platforms;
    uint32_t architectures;
};

struct Negotiation {
    int32_t status;
    uint64_t active;
};

struct Decision {
    int32_t status;
    uint64_t generation;
    uint32_t bits;
    uint32_t text_len;
    uint32_t font_len;
};

using Decide = Decision(__cdecl*)(
    void*,
    const uint16_t*,
    uint32_t,
    uint16_t*,
    uint32_t,
    uint16_t*,
    uint32_t
);

struct Host {
    uint32_t size;
    void* context;
    Decide decide;
    uint32_t(__cdecl* characters)(void*, uint16_t*, uint32_t);
};

struct Api {
    uint32_t size;
    Descriptor descriptor;
    Negotiation(__cdecl* negotiate)(uint64_t, uint64_t);
    Negotiation(__cdecl* activate)(const Host*, uint64_t, uint64_t);
    int32_t(__cdecl* deactivate)();
    void(__cdecl* refresh)();
};

struct TextEvent {
    uint32_t size;
    uint32_t kind;
    uint64_t surface;
    uint64_t run;
    uint64_t epoch;
    uint32_t ordinal;
    const uint16_t* source;
    uint32_t length;
};

using DecideText = Decision(__cdecl*)(
    void*,
    const TextEvent*,
    uint16_t*,
    uint32_t,
    uint16_t*,
    uint32_t
);
using EnterTextScope = uint64_t(__cdecl*)(void*);
using LeaveTextScope = void(__cdecl*)(void*, uint64_t);

struct TextHost {
    uint32_t size;
    uint32_t version;
    void* context;
    DecideText decide;
    EnterTextScope enter;
    LeaveTextScope leave;
};

constexpr uint64_t GS_FEATURE_TEXT_OBSERVE = 1ull << 0;
constexpr uint64_t GS_FEATURE_TEXT_REPLACE = 1ull << 1;
constexpr uint32_t GS_PLATFORM_WINDOWS = 1u << 0;
constexpr uint32_t GS_ARCH_X86 = 1u << 0;
constexpr uint32_t GS_ARCH_X86_64 = 1u << 1;
constexpr uint32_t GS_TEXT_EVENT_DRAW = 1;
constexpr uint32_t GS_TEXT_EVENT_OBSERVE = 3;
constexpr int32_t GS_STATUS_OK = 0;
constexpr int32_t GS_STATUS_UNSUPPORTED_FEATURE = 1;
constexpr int32_t GS_STATUS_UNAUTHORIZED_FEATURE = 2;
constexpr int32_t GS_STATUS_ACTIVATION_FAILED = 3;
constexpr int32_t GS_STATUS_INVALID_HOST = 4;

static_assert(sizeof(FixedUtf8) == 98);
static_assert(sizeof(Descriptor) == 136);
static_assert(sizeof(Decision) == 32);
static_assert(sizeof(Host) == (sizeof(void*) == 8 ? 32 : 16));
static_assert(sizeof(Api) == (sizeof(void*) == 8 ? 176 : 160));
static_assert(sizeof(TextEvent) == (sizeof(void*) == 8 ? 56 : 48));
static_assert(sizeof(TextHost) == (sizeof(void*) == 8 ? 40 : 24));
