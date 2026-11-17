uniform float time;
uniform vec2 iResolution;

uniform float freq;      // Default : 12.32  Min :  1.0   Max :  16.0
uniform float radius1;   // Default :  0.1   Min :  0.01  Max :   0.5
uniform float radius2;   // Default :  0.67  Min :  0.01  Max :   1.99
uniform float rate;      // Default :  0.01  Min : -0.1   Max :   0.1
uniform float loops;     // Default : 58.93  Min :  1.0   Max : 200.0
uniform float thickness; // Default :  0.005 Min :  0.001 Max :   0.01
uniform float nudge;     // Default :  0.875 Min :  0.0   Max :   1.0
uniform float hue;       // Default :  3.0   Min :  1.0   Max :   10.0

//////////////////////////////////////////////
// SpirographSpectrum by mojovideotech
//
// based on :
// www.shadertoy.com/\view/\lddGD4
//
// Creative Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License.
/////////////////////////////////////////////


#define 	twpi  6.283185307179586  	// two pi, 2*pi
#define 	pi   	3.141592653589793 	// pi


void main() {

	//vec2 uv = (gl_FragCoord.xy-RENDERSIZE.xy*0.5)  / RENDERSIZE.y - center ;

  vec2 res = iResolution.xy;
  vec2 uv = ((1.0 * gl_FragCoord.xy - res * 0.5) / res.x);
    
  float ang = (atan(-uv.y,-uv.x)/atan(1.0)*0.125+0.5)+time*rate;
  float len = length(uv);
  vec3 col = vec3(0.0);
  for(float i = 0.0;i<200.;i+=1.0)
    {
    	float L = floor(loops);
    	float F = ceil(freq);
    	if (i >= L) break;
        float tag = (ang+i)*twpi;
        float tln = (cos((ang+i*(1.0-nudge))*F)*0.5+0.5)*(radius2-radius1)+radius1;
        vec2 pos = normalize(uv)*tln;
    	col += smoothstep(thickness,0.0,distance(uv,pos)*pow(length(uv),0.001))*(cos(tag+vec3(0.0,twpi,2.0*twpi)/hue)*0.5+0.5);
    }

  // make black transparent
  if (col.r == 0 && col.g == 0 && col.b == 0) {
    gl_FragColor = vec4(col, 0.0); // Set alpha to 0 for black
  } else {
    gl_FragColor = vec4(col,1.0); // Keep the original color
  }

}
