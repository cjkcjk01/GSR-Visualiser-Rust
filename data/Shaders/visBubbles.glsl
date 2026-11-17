////////////////////////////////////////////////////////////
// Bubbles  (SaturdayShader Week 21 : Cosplay)
// by Joseph Fiola (http://joefiola.com) - 2016-01-09
// Modified for Dynamic Iridescent Bubble Rims
//
// Converted from ISF (RENDERSIZE/TIME) to plain GLSL (iResolution/time)
////////////////////////////////////////////////////////////

uniform float dotSize;          // Default : 0.02  Min : 0.0    Max : 0.1
uniform float rimThickness;     // Default : 0.15  Min : 0.01   Max : 0.5
uniform float iteration;        // Default : 60.0  Min : 0.0    Max : 100.0
uniform float xAmp;             // Default : 0.3   Min : -1.0   Max : 1.0
uniform float yAmp;             // Default : 0.1   Min : -1.0   Max : 1.0
uniform float xFactor;          // Default : 0.2   Min : 0.0    Max : 10.0
uniform float yFactor;          // Default : 0.2   Min : 0.0    Max : 10.0
uniform float speed;            // Default : 0.05  Min : 0.0    Max : 0.1
uniform float rotateCanvas;     // Default : 0.0   Min : -3.14159 Max : 3.14159
uniform vec2  pos;              // Default : 0.5, 0.5

uniform vec2  iResolution;
uniform float time;

// Rotation function
vec2 rot(vec2 uv, float a) {
  return vec2(uv.x*cos(a)-uv.y*sin(a), uv.y*cos(a)+uv.x*sin(a));
}

// Generates a bubble shell with an externally controlled rim thickness
float bubble(vec2 uv, float size, float thickness) {
  float dist = length(uv);

  // Calculate the inner boundary based on the thickness factor
  float innerRim = size * (1.0 - thickness);
  float fadeStart = innerRim - (size * 0.05); // Smooth fallback for the inside edge

  // Creates the hollow ring geometry
  float rim = smoothstep(size, innerRim, dist) * smoothstep(fadeStart, innerRim, dist);

  // Adds a tiny, delicate center reflection highlight
  float specular = smoothstep(size * 0.12, 0.0, dist) * 0.4;

  return rim + specular;
}

// Cosine gradient generator for thin-film iridescence
vec3 getIridescence(float t) {
  vec3 a = vec3(0.5, 0.5, 0.5);
  vec3 b = vec3(0.5, 0.5, 0.5);
  vec3 c = vec3(1.0, 1.0, 1.0);
  vec3 d = vec3(0.0, 0.33, 0.67); // Phase shifts that yield liquid oil-slick wavelengths
  return a + b * cos(6.28318 * (c * t + d));
}

void main() {
  vec2 uv = gl_FragCoord.xy / iResolution;
  uv -= pos;
  uv.x *= iResolution.x / iResolution.y;

  vec3 color = vec3(0.0);
  uv = rot(uv, rotateCanvas);

  float i = 0.0;
  for (int _i = 0; _i < 100; _i++) {
    i = float(_i);
    if (iteration < i) break;

    // Trigonometric orbital movement pathing
    vec2 st = uv - vec2(cos(i * xFactor * (time * speed)) * xAmp, sin(i * yFactor * (time * speed)) * yAmp);

    // Dynamically scale overall bubble diameter over loop iterations
    float currentSize = dotSize * (i * 0.05 + 1.0);

    // Process bubble shape using the custom user-controlled thickness variable
    float intensity = bubble(st, currentSize, rimThickness);

    // Dynamic Color Binding: Multi-layered values dictate the current wavelength phase
    // 1. (i / iteration) binds color variance to the loop index step
    // 2. (length(st) * 2.5) ties color shifting structurally to the distance from bubble center
    // 3. (time * speed) acts as a constant temporal pulse pushing the palette forward
    float colorShift = (i / iteration) + (length(st) * 2.5) - (time * speed);
    vec3 bubbleColor = getIridescence(colorShift);

    // Blend colors into the output buffer
    color += bubbleColor * intensity;
  }

  // Output final color with solid alpha channel
  gl_FragColor = vec4(color, 1.0);
}
