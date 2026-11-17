////////////////////////////////////////////////////////////
// Orbs
//
// Audio-reactive kaleidoscopic orbs. Original: https://www.shadertoy.com/view/WtGfRw
// Credit: https://www.shadertoy.com/user/yozic
//
// Converted from ISF (RENDERSIZE/TIME) to plain GLSL (iResolution/time).
// orbSize is a fixed constant in the original (not an ISF input); gloop and
// sides are declared as ISF inputs but never referenced by the shader, so
// they're dropped here.
////////////////////////////////////////////////////////////

#define PI 3.141592
#define orbs 20.
#define orbSize 6.46

uniform float zoom;         // Default : 0.07
uniform float contrast;     // Default : 0.13  (kept in 0.08-0.16 by the caller)
uniform float radius;       // Default : 11.0  Max : 100.0
uniform float colorShift;   // Default : 10.32 Max : 100.0
uniform float rotation;     // Default : 1.0
uniform float sinMul;       // Default : 0.0   Min : -10.0 Max : 10.0
uniform float cosMul;       // Default : 2.38  Min : -10.0 Max : 10.0
uniform float yMul;         // Default : 0.0
uniform float xMul;         // Default : 0.28
uniform float xSpeed;       // Default : 0.0
uniform float ySpeed;       // Default : 0.0
uniform float yDivide;      // Default : 4.99
uniform float xDivide;      // Default : 6.27

uniform vec2  iResolution;
uniform float time;

vec4 orb(vec2 uv, float s, vec2 p, vec3 color, float c) {
  return pow(vec4(s / length(uv + p) * color, 1.0), vec4(c));
}

mat2 rotate(float a) {
  return mat2(cos(a), -sin(a), sin(a), cos(a));
}

void main() {
  vec4 fragColor = vec4(0.0);

  vec2 uv = (2.0 * gl_FragCoord.xy - iResolution) / iResolution.y;
  uv *= zoom;
  uv /= dot(uv, uv);
  uv *= rotate(rotation * time / 10.0);

  for (float i = 0.0; i < orbs; i++) {
    uv.x += sinMul * sin(uv.y * yMul + time * xSpeed) + cos(uv.y / yDivide - time);
    uv.y += cosMul * cos(uv.x * xMul - time * ySpeed) - sin(uv.x / xDivide - time);
    float t = i * PI / orbs * 2.0;
    float x = radius * tan(t);
    float y = radius * cos(t + time / 10.0);
    vec2 position = vec2(x, y);
    vec3 color = cos(0.02 * uv.x + 0.02 * uv.y * vec3(-2, 0, -1) * PI * 2.0 / 3.0 + PI * (i / colorShift)) * 0.5 + 0.5;
    fragColor += 0.65 - orb(uv, orbSize, position, 1.0 - color, contrast);
  }

  fragColor.a = 1.0;
  gl_FragColor = fragColor;
}
