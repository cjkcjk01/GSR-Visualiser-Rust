/*{
	"DESCRIPTION": "Your shader description",
	"CREDIT": "by you",
	"CATEGORIES": [
		"Your category"
	],
	"INPUTS": [
		{
			"NAME": "iChannel0",
			"TYPE": "image"
		},
		{
			"NAME": "amount",
			"TYPE": "float",
			"DEFAULT": 0.03,
			"MIN": 0.0,
			"MAX": 1.0
		},
		{
			"NAME": "level",
			"TYPE": "float",
			"DEFAULT": 0.5,
			"MIN": 0.0,
			"MAX": 1.0
		},
		{
			"NAME": "freq",
			"TYPE": "float",
			"DEFAULT": 1.0,
			"MIN": 0.0,
			"MAX": 10.0
		}
      	]
}*/
precision highp float;

uniform float amount;
uniform float level;
uniform float freq;
uniform float iGlobalTime;
uniform vec2 iResolution;
uniform sampler2D texture;

vec3 iResolution2 = vec3(iResolution, 1.);

// set up constants, effects ripple behaviour
//float amount = 0.103;
//float level = 6.0;

// function warp x or y coordinate based on time and sin
float warp( float a )
{
    float timeMod = iGlobalTime + a;
    return fract(a + cos( timeMod*freq * level*10.0) * amount + sin( timeMod * level*10.0/4.1317 ) * amount); //doubled the function. fract makes it tile!
}

// main thing
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // get 0..1 uv coords instead of pixel coords
    vec2 uv = (fragCoord.xy / iResolution2.xy);
    
    // warp each uv coord seperately using our cool function
    uv.x = warp(uv.x);
    uv.y = warp(uv.y);
    
    // return the color from the texture with the warped uv
	fragColor = texture2D(texture, uv);
}

void main(void) {
    mainImage(gl_FragColor, gl_FragCoord.xy);
}