////////////////////////////////////////////////////////////
// Neon Edge Spark Dissolve
//
// Edge pixels launch as wind-blown sparks. Based on original by VIDVOX.
//
// Converted from ISF (isf_FragNormCoord/IMG_NORM_PIXEL/RENDERSIZE/TIME) to
// this project's PROCESSING_TEXTURE_SHADER filter() convention.
////////////////////////////////////////////////////////////

#ifdef GL_ES
precision mediump float;
precision mediump int;
#endif

#define PROCESSING_TEXTURE_SHADER

varying vec4 vertTexCoord;
uniform sampler2D texture;

uniform float intensity;     // Default : 10.0  Min : 0.0   Max : 10.0
uniform float dissolve;      // Default :  0.75 Min : 0.0   Max :  1.0
uniform float pixelSize;     // Default :  0.001 Min : 0.001 Max : 0.01
uniform float windSpeed;     // Default :  0.1  Min : 0.01  Max :  6.0
uniform float windStrength;  // Default :  0.15 Min : 0.0   Max :  0.5
uniform float windAngle;     // Default :  0.0  Min : 0.0   Max :  6.2832
uniform float r;             // Default :  1.0  Min : 0.0   Max :  1.0
uniform float g;             // Default :  0.4  Min : 0.0   Max :  1.0
uniform float b;             // Default :  0.64 Min : 0.0   Max :  1.0
uniform bool  useRainbow;    // Default : false
uniform float hueSpeed;      // Default :  0.5  Min : 0.0   Max :  2.0

uniform vec2  iResolution;
uniform float time;

vec3 hsv2rgb(vec3 c) {
  vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
  vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
  return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

float gray(vec4 n) {
  return (n.r + n.g + n.b) / 3.0;
}

float hash2(vec2 p) {
  p = fract(p * vec2(127.1, 311.7));
  p += dot(p, p + 19.19);
  return fract(p.x * p.y);
}

float edgeAt(vec2 coord) {
  float offset = 0.002;
  float gL = gray(texture2D(texture, coord + vec2(-offset, 0.0)));
  float gR = gray(texture2D(texture, coord + vec2( offset, 0.0)));
  float gA = gray(texture2D(texture, coord + vec2(0.0,  offset)));
  float gB = gray(texture2D(texture, coord + vec2(0.0, -offset)));
  return sqrt((gR - gL) * (gR - gL) + (gA - gB) * (gA - gB));
}

// Returns brightness of one spark slot for this cell
float sparkSlot(vec2 cell, float slotSeed, vec2 windDir, float aspect) {
  float rndA = hash2(cell + slotSeed);
  float rndB = hash2(cell + slotSeed + 3.7);
  float rndC = hash2(cell + slotSeed + 7.3);
  float rndD = hash2(cell + slotSeed + 13.5);

  // Spread direction in a cone around wind
  float spreadAngle = (rndB - 0.5) * 1.4;
  float cosS = cos(spreadAngle);
  float sinS = sin(spreadAngle);
  vec2 sparkDir = vec2(
    windDir.x * cosS - windDir.y * sinS,
    windDir.x * sinS + windDir.y * cosS
  );

  // Cycling age - each slot has its own phase offset so they fire at different times
  float phase = fract(time * windSpeed * 0.3 + rndA);
  float activeWindow = 0.3 + rndD * 0.3;
  float sparkAge = clamp(phase / activeWindow, 0.0, 1.0);

  // Where did this spark come FROM at this age? (reverse trace back to source)
  vec2 cellCenter = vec2(
    cell.x + pixelSize * 0.5,
    (cell.y + pixelSize * 0.5) * aspect
  );
  vec2 sourceUV = cellCenter - sparkDir * sparkAge * windStrength;
  sourceUV = clamp(sourceUV, 0.0, 1.0);

  // How strong is the edge at the source?
  float edgeSample = edgeAt(sourceUV);

  // Bright when young, fades as it ages
  float sparkFade = exp(-sparkAge * 3.0) * (1.0 - sparkAge);

  // Only contribute if there's actually an edge at source
  return edgeSample * intensity * sparkFade * (rndC * 0.4 + 0.6);
}

void main(void) {
  vec2 uv      = vertTexCoord.st;
  float aspect = iResolution.x / iResolution.y;
  vec2 windDir = vec2(cos(windAngle), sin(windAngle));

  // Snap to square pixel grid
  vec2 uvA    = vec2(uv.x, uv.y / aspect);
  vec2 cell   = floor(uvA / pixelSize) * pixelSize;

  // 8 independent spark slots per cell - no loop, just 8 inlined calls
  // Each slot has a unique seed so they behave independently
  float best = 0.0;
  float s;

  s = sparkSlot(cell, 0.00, windDir, aspect); if (s > best) best = s;
  s = sparkSlot(cell, 0.13, windDir, aspect); if (s > best) best = s;
  s = sparkSlot(cell, 0.27, windDir, aspect); if (s > best) best = s;
  s = sparkSlot(cell, 0.41, windDir, aspect); if (s > best) best = s;
  s = sparkSlot(cell, 0.55, windDir, aspect); if (s > best) best = s;
  s = sparkSlot(cell, 0.69, windDir, aspect); if (s > best) best = s;
  s = sparkSlot(cell, 0.83, windDir, aspect); if (s > best) best = s;
  s = sparkSlot(cell, 0.97, windDir, aspect); if (s > best) best = s;

  float sparkBright = best * dissolve;
  float solidBright = edgeAt(uv) * intensity * (1.0 - dissolve);
  float finalLum    = max(solidBright, sparkBright);

  vec3 tint;
  if (useRainbow) {
    float hue = fract(time * hueSpeed * 0.2);
    tint = hsv2rgb(vec3(hue, 0.7, 1.0));
  } else {
    tint = vec3(r, g, b);
  }

  gl_FragColor = vec4(finalLum * tint, finalLum);
}
