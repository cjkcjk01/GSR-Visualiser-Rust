////////////////////////////////////////////////////////////
// OpArt
//
// Moving distorted lines in a circle, inspired by Op Art.
//
// Converted from ISF (isf_FragNormCoord/RENDERSIZE/TIME) to plain GLSL
// (gl_FragCoord/iResolution/time)
////////////////////////////////////////////////////////////

uniform float lineSpacing;      // Default : 90.0  Min : 10.0 Max : 200.0
uniform float speed;            // Default :  1.0  Min :  0.0 Max :   5.0
uniform float distortionAmount; // Default :  0.15 Min :  0.0 Max :   1.0
uniform vec4  color1;           // Default : 0.0, 0.0, 0.0, 1.0
uniform vec4  color2;           // Default : 1.0, 1.0, 1.0, 1.0
uniform vec4  bgColor;          // Default : 0.94, 0.94, 0.94, 1.0

uniform vec2  iResolution;
uniform float time;

void main() {
  // Normalise screen coordinates and correct for aspect ratio, so the
  // circle stays circular rather than squashing into an ellipse.
  vec2 st = gl_FragCoord.xy / iResolution.xy;
  vec2 uv = st - 0.5;
  uv.x *= iResolution.x / iResolution.y;

  // Circular mask (radius 0.45, soft edge)
  float radius = length(uv);
  float circleMask = smoothstep(0.45, 0.445, radius);

  // Line distortion and movement. time makes the wave shift gradually.
  float wave = sin(uv.y * 12.0 - time * speed) * 0.5 +
               cos(uv.y * 4.0 + uv.x * 3.0 + time * (speed * 0.5)) * 0.5;

  // Distort the x coordinate by the wave
  float distortedX = uv.x + wave * distortionAmount;

  // Line pattern (sine wave sets the spacing)
  float lineVal = sin(distortedX * lineSpacing);

  // Sharpen the line boundaries into crisp zebra stripes
  float sharpLines = smoothstep(-0.05, 0.05, lineVal);

  // Blend the two line colours
  vec4 patternColor = mix(color1, color2, sharpLines);

  // Pattern inside the circular mask, background colour outside it
  vec4 finalColor = mix(bgColor, patternColor, circleMask);

  gl_FragColor = finalColor;
}
