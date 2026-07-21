#version 150 core
in vec2 Texcoord;
out vec4 outColor;
uniform sampler2D tex;
uniform float t;

// Pillow distortion strength (higher = more curved screen)
const vec2 barrel_distortion = vec2(0.09, 0.09);

vec2 warp(vec2 uv) {
  vec2 coord = uv - 0.5;
  // Apply the polynomial curve to mimic a rounded CRT glass pane
  coord += coord * (coord.x * coord.x * barrel_distortion.x + coord.y * coord.y * barrel_distortion.y);
  return coord + 0.5;
}

void main() {
  vec2 uv = warp(Texcoord);

  // Rounded corners
  const float corner_radius = 0.035;

  vec2 p = abs(uv - 0.5) - vec2(0.5 - corner_radius);
  vec2 q = max(p, 0.0);

  float d = length(q) - corner_radius;

  if(d > 0.0) {
    outColor = vec4(0.0, 0.0, 0.0, 1.0);
    return;
  }
  
  // Fetch primary color sample
  vec4 base_color = texture(tex, uv);

  // Fake Phosphor Glow/Bleed (sample slightly adjacent pixels)
  vec4 glow = texture(tex, uv + vec2(0.0015, 0.0)) * 0.2;
  glow += texture(tex, uv - vec2(0.0015, 0.0)) * 0.2;
  vec4 final_rgb = base_color + glow;

  // Apply Scanline Intensity Wave (based on internal 400 vertical text lines)
  float scanline = sin(uv.y * 440.0 * 3.14159) * 0.08;
  final_rgb.rgb -= scanline;

  outColor = final_rgb + t;
}
