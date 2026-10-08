// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// colour: a site's picture in the screen's own colours, and calmer when
// asked (docs/colour.md). Each pixel is looked up in two tables made by
// colour.rs: the cube, 33 × 33 × 33 entries packed as 33 tiles of 33 × 33
// side by side (blue picks the tile), holding the screen's linear values;
// then the curve, 1,024 entries read at u = y^(1/2.4), turning them into the
// screen's numbers. Baked into colour.frag.qsb by tools/make-shaders.sh.

#version 440

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    // Entries of the cube on each side.
    float tableSize;
};

layout(binding = 1) uniform sampler2D source;
layout(binding = 2) uniform sampler2D cube;
layout(binding = 3) uniform sampler2D curve;

// The cube at sRGB numbers c: bilinear within a tile (red, green), linear between two tiles (blue).
vec3 lookup(vec3 c)
{
    float n = tableSize;
    float b = c.b * (n - 1.0);
    float b0 = min(floor(b), n - 2.0);
    float f = b - b0;
    vec2 rg = (c.rg * (n - 1.0) + 0.5) / vec2(n * n, n);
    vec3 low = texture(cube, rg + vec2(b0 / n, 0.0)).rgb;
    vec3 high = texture(cube, rg + vec2((b0 + 1.0) / n, 0.0)).rgb;
    return mix(low, high, f);
}

void main()
{
    vec4 pixel = texture(source, qt_TexCoord0);
    // Qt Quick's textures hold premultiplied colours: the page's own numbers first.
    vec3 c = pixel.a > 0.0 ? clamp(pixel.rgb / pixel.a, 0.0, 1.0) : vec3(0.0);
    vec3 linear = clamp(lookup(c), 0.0, 1.0);
    vec3 u = (pow(linear, vec3(1.0 / 2.4)) * 1023.0 + 0.5) / 1024.0;
    vec3 screen = vec3(texture(curve, vec2(u.r, 0.5)).r, texture(curve, vec2(u.g, 0.5)).g, texture(curve, vec2(u.b, 0.5)).b);
    fragColor = vec4(screen * pixel.a, pixel.a) * qt_Opacity;
}
