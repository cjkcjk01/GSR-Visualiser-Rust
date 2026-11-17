////////////////////////////////////////////////////////////
// Voronoi Slice
//
// Voronoi cellular slicing and rotation effect. Credit: stk.
//
// Converted from ISF (IMG_NORM_PIXEL/RENDERSIZE/TIME) to this project's
// PROCESSING_TEXTURE_SHADER filter() convention.
////////////////////////////////////////////////////////////

#ifdef GL_ES
precision mediump float;
precision mediump int;
#endif

#define PROCESSING_TEXTURE_SHADER
#define PI 3.14159265359

varying vec4 vertTexCoord;
uniform sampler2D texture;

uniform float cellScale;      // Default : 8.0  Min : 2.0  Max : 30.0
uniform float gridLineWidth;  // Default : 0.02 Min : 0.0  Max :  0.1
uniform float speed;          // Default : 1.0  Min : 0.0  Max :  5.0
uniform float randomness;     // Default : 0.5  Min : 0.0  Max :  1.0
uniform vec2  centerPoint;    // Default : 0.5, 0.5

uniform vec2  iResolution;
uniform float time;

// Vector pseudo-random generator 2D
vec2 hash2(vec2 p) {
  p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
  return fract(sin(p) * 43758.5453123);
}

void main() {
  // 1. Normalise coordinates around the centre point
  vec2 st = (gl_FragCoord.xy - centerPoint * iResolution) / iResolution.y;

  // 2. Scale coordinates onto the Voronoi grid
  vec2 stScaled = st * cellScale;
  vec2 i_st = floor(stScaled);
  vec2 f_st = fract(stScaled);

  // 3. Find the nearest cell centre and the distance to it
  vec2 closestCell;
  vec2 closestPoint;
  float minDist = 8.0;

  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 neighbor = vec2(float(x), float(y));
      vec2 point = hash2(i_st + neighbor);

      // Animate the cell point's movement, driven by randomness
      point = 0.5 + 0.5 * sin(time * speed * 0.5 + 6.2831 * point * randomness);

      vec2 diff = neighbor + point - f_st;
      float dist = length(diff);

      if (dist < minDist) {
        minDist = dist;
        closestPoint = point;
        closestCell = i_st + neighbor;
      }
    }
  }

  // 4. Distance to the cell boundary, for the grid lines
  float edgeDist = 8.0;
  for (int y = -2; y <= 2; y++) {
    for (int x = -2; x <= 2; x++) {
      vec2 neighbor = vec2(float(x), float(y));
      vec2 point = hash2(i_st + neighbor);
      point = 0.5 + 0.5 * sin(time * speed * 0.5 + 6.2831 * point * randomness);

      vec2 diff = neighbor + point - f_st;
      vec2 cellToCenter = (closestCell - (i_st + neighbor));

      if (dot(cellToCenter, cellToCenter) > 0.0001) {
        vec2 centerDiff = (neighbor + point) - (closestCell + closestPoint - i_st);
        float d = dot(diff - centerDiff * 0.5, normalize(centerDiff));
        edgeDist = min(edgeDist, d);
      }
    }
  }

  // 5. Random rotation per Voronoi cell
  float cellRand = hash2(closestCell).x;
  float direction = (cellRand > 0.5) ? 1.0 : -1.0;
  float randSpeed = mix(1.0, hash2(closestCell).y * 2.0 + 0.5, randomness);
  float angle = time * speed * direction * randSpeed;

  // 6. Apply the rotation to the UV coordinates inside the Voronoi cell
  vec2 uv = (gl_FragCoord.xy - centerPoint * iResolution) / iResolution;

  // Local cell centre in normalised UV coordinates
  vec2 cellCenterUV = ((closestCell + closestPoint) / cellScale) * (iResolution.y / iResolution.x);
  vec2 localUV = uv - cellCenterUV;

  float cosA = cos(angle);
  float sinA = sin(angle);
  float aspect = iResolution.x / iResolution.y;

  localUV.x *= aspect;
  vec2 rotatedLocalUV;
  rotatedLocalUV.x = localUV.x * cosA - localUV.y * sinA;
  rotatedLocalUV.y = localUV.x * sinA + localUV.y * cosA;
  rotatedLocalUV.x /= aspect;

  vec2 rotatedUV = rotatedLocalUV + cellCenterUV + centerPoint;

  // 7. Sample the source image at the rotated coordinate
  vec4 color = texture2D(texture, rotatedUV);

  // 8. Voronoi cell boundary grid lines
  float gridMask = smoothstep(0.0, gridLineWidth, edgeDist);

  // 9. Outer circular mask
  float r = length(st);
  float maxRadius = 0.5;
  float outerMask = smoothstep(maxRadius, maxRadius - 0.005, r);

  gl_FragColor = vec4(color.rgb * gridMask, color.a) * outerMask;
}
