#include <metal_stdlib>
using namespace metal;

struct VertexIn {
    float2 position [[attribute(0)]];
    float3 color [[attribute(1)]];
};

struct VSOut {
    float4 position [[position]];
    float3 color;
};

vertex VSOut vertex_main(const device VertexIn* vertices [[buffer(0)]], uint vid [[vertex_id]]) {
    VSOut out;
    out.position = float4(vertices[vid].position, 0.0, 1.0);
    out.color = vertices[vid].color;
    return out;
}

fragment float4 fragment_main(VSOut in [[stage_in]]) {
    return float4(in.color, 1.0);
}
