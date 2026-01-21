#include <metal_stdlib>
using namespace metal;

struct VertexIn {
    float3 position [[attribute(0)]];
    float3 color [[attribute(1)]];
};

struct Uniforms {
    float4x4 view_proj;
};

struct VSOut {
    float4 position [[position]];
    float3 color;
};

vertex VSOut vertex_main(
    const device VertexIn* vertices [[buffer(0)]],
    constant Uniforms& uniforms [[buffer(1)]],
    uint vid [[vertex_id]]
) {
    VSOut out;
    out.position = uniforms.view_proj * float4(vertices[vid].position, 1.0);
    out.color = vertices[vid].color;
    return out;
}

fragment float4 fragment_main(VSOut in [[stage_in]]) {
    return float4(in.color, 1.0);
}
