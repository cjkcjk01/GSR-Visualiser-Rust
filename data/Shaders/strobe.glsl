/*{
    "CREDIT": "by kawaiiidesu",
    "DESCRIPTION": "",
    "CATEGORIES": [
        "generator"
    ],
    "INPUTS": [
        {
            "NAME": "colorInput",
            "TYPE": "color",
            "DEFAULT": [
                1.0,
                1.0,
                1.0,
                1.0
            ]
        },
     {
      "NAME": "freq",
      "TYPE": "float",
      "MAX": 0.1,
      "MIN": 0.01,
      "DEFAULT": 0.05
    }
    ]
}*/
const vec4 colorInput = vec4(1.0, 1.0, 1.0, 1.0); 
uniform float freq;
uniform float time;

void main() {
    float t;
    t=ceil(cos(time/freq));
    gl_FragColor = colorInput*t;
}
