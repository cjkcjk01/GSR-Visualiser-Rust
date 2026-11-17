////////////////////////////////////////////////////////////
// Isometric Facet Hatching (Crisp Lines)
//
// Isometric triangular facet hatching with sharp, non-blurred vector-style lines.
//
// Converted from ISF (RENDERSIZE) to plain GLSL (iResolution)
////////////////////////////////////////////////////////////

#ifdef GL_ES
precision mediump float;
#endif

#define PI 3.14159265359
#define SQRT3 1.73205080757

uniform float scale;            // Default : 4.3   Min : 2.0    Max : 15.0
uniform float lineDensity;      // Default : 18.0  Min : 5.0    Max : 50.0
uniform float borderThickness;  // Default : 0.01  Min : 0.01   Max : 0.2
uniform float clusterSize;      // Default : 0.8   Min : 0.0    Max : 2.0
uniform float randomness;       // Default : 0.7   Min : 0.0    Max : 1.0
uniform vec4  backgroundColor;  // Default : 0.95, 0.96, 0.97, 1.0
uniform vec4  lineColor;        // Default : 0.08, 0.12, 0.20, 1.0

uniform vec2 iResolution;

// Grid-based hash
float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

// Isometric triangle grid function
vec4 simplexGrid(vec2 p, out float edgeDist) {
  vec2 s = vec2(p.x - p.y / SQRT3, p.y * 2.0 / SQRT3);

  vec2 i = floor(s);
  vec2 f = fract(s);

  float triType = (f.x + f.y > 1.0) ? 1.0 : 0.0;

  float d1 = f.x;
  float d2 = f.y;
  float d3 = abs(1.0 - f.x - f.y);

  edgeDist = min(min(d1, d2), d3);

  vec2 triID = i + vec2(triType);

  return vec4(f, triID);
}

// Crisp (non-blurred) hatching line generator
float drawHatchingCrisp(vec2 p, float angle, float density, float thickness) {
  float c = cos(angle);
  float s = sin(angle);
  float pos = p.x * c + p.y * s;

  float wave = fract(pos * density);

  // step gives a genuinely crisp/solid edge rather than a blurred one
  float val = abs(wave - 0.5) * 2.0;
  return step(val, thickness);
}

void main() {
  // Normalise UV (centred at 0,0)
  vec2 uv = (gl_FragCoord.xy * 2.0 - iResolution.xy) / iResolution.y;

  float distFromCenter = length(uv);
  vec2 st = uv * scale;

  float edgeDist;
  vec4 grid = simplexGrid(st, edgeDist);
  vec2 triID = grid.zw;

  float randVal = hash(triID);
  float randVal2 = hash(triID + vec2(15.4, 7.8));

  float clusterMask = step(randVal * 0.4 + (1.0 - clusterSize), distFromCenter);

  // Hatching line direction (0 degrees, 60 degrees, 120 degrees)
  float angleChoice = floor(randVal * 3.0);
  float angle = 0.0;
  if (angleChoice == 1.0) angle = PI / 3.0;
  else if (angleChoice == 2.0) angle = 2.0 * PI / 3.0;

  float lineThickness = mix(0.2, 0.7, pow(randVal2, 2.0) * randomness);

  // 1. Draw the crisp hatching lines
  float lines = drawHatchingCrisp(st, angle, lineDensity, lineThickness);

  // 2. Draw the crisp (non-blurred) triangle border lines
  float border = step(edgeDist, borderThickness);

  // Combine the fill lines and the border lines
  float pattern = max(lines, border);

  // Hide anything outside of the main cluster
  if (clusterMask > 0.5) {
    pattern = 0.0;
  }

  vec3 color = mix(backgroundColor.rgb, lineColor.rgb, pattern);

  gl_FragColor = vec4(color, 1.0);
}
