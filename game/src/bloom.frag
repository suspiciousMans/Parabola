#version 330 core
// Tonemap composite with a cheap one-pass bloom: a golden-angle spiral of
// taps around each pixel, keeping only the part of the HDR colour above 1.
in vec2 vUV;
out vec4 FragColor;

uniform sampler2D uSourceTex;
uniform float uExposure;
uniform vec3 uTintColor;
uniform float uTintStrength;
uniform float uColorLevels;
uniform float uDitherStrength;

vec3 aces(vec3 x) {
    const float a = 2.51, b = 0.03, c = 2.43, d = 0.59, e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0);
}

vec3 bright(vec3 c) {
    float l = max(c.r, max(c.g, c.b));
    return c * max(l - 1.4, 0.0) / max(l, 1e-4);
}

void main() {
    vec3 base = texture(uSourceTex, vUV).rgb;
    vec2 px = 1.0 / vec2(textureSize(uSourceTex, 0));
    vec3 glow = vec3(0.0);
    float wsum = 0.0;
    for (int i = 0; i < 48; i++) {
        float t = (float(i) + 0.5) / 48.0;
        float r = t * t * 20.0;
        float a = float(i) * 2.39996;
        vec2 o = vec2(cos(a), sin(a)) * r * px;
        float w = 1.0 - t * 0.85;
        glow += bright(texture(uSourceTex, vUV + o).rgb) * w;
        wsum += w;
    }
    glow /= wsum;
    vec3 hdr = base + glow * 1.4;
    vec3 mapped = aces(hdr * uExposure);
    vec3 srgb = pow(mapped, vec3(1.0 / 2.2));
    // cinematic grade: cool shadows, warm highlights, soft vignette
    float lum = dot(srgb, vec3(0.299, 0.587, 0.114));
    srgb = mix(srgb * vec3(0.92, 1.0, 1.1), srgb * vec3(1.06, 1.0, 0.94), smoothstep(0.2, 0.8, lum));
    float vig = smoothstep(1.05, 0.25, length((vUV - 0.5) * vec2(1.25, 1.0)));
    srgb *= mix(0.5, 1.0, vig);
    srgb = mix(srgb, uTintColor, uTintStrength);
    FragColor = vec4(clamp(srgb, 0.0, 1.0), 1.0);
}
