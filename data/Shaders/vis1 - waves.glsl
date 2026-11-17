// Waves
// https://editor.isf.video/shaders/68e7cad5083089001a6861ee

uniform float speed;         // Default :  0.5  Min :  0.0   Max :   2.0
uniform float lineCount;     // Default :  8.0  Min :  1.0   Max :  15.0
uniform float waveAmplitude; // Default :  0.3  Min :  0.1   Max :   1.0
uniform float lineThickness; // Default :  0.05 Min :  0.001 Max :   0.015
uniform float glowIntensity; // Default :  1.0  Min :  0.0   Max :   2.0
uniform float colorMix;      // Default :  0.5  Min :  0.0   Max :   1.0
uniform vec4 color1; // Default 0.0,0.8,1.0,1.0
uniform vec4 color2; // Default 0.0,1.0,0.6,1.0
uniform vec4 color3; // Default 0.3,0.9,0.8,1.0

uniform vec2 iResolution;
uniform float time;

void main() {

    vec2 res = iResolution.xy;
    vec2 uv = ((1.0 * gl_FragCoord.xy - 0.5 * res) / res.x);

    //vec2 p = uv * 2.0 - 1.0;
    vec2 p = uv * 2.0; // This affects the vertical positioning of the waves
    p.x *= res.x / res.y;
    
    float t = time * speed;
    vec3 col = vec3(0.0);
    
    // Créer plusieurs lignes fluides
    for(float i = 0.0; i < 15.0; i++) {
        if(i >= lineCount) break;
        float offset = i / lineCount;
        
        // Position verticale de base pour chaque ligne
        float baseY = (offset - 0.5) * 1.8;
        
        // Créer des ondulations multiples
        float wave1 = sin(p.x * 2.0 + t + offset * 6.28) * waveAmplitude;
        float wave2 = sin(p.x * 3.5 - t * 0.7 + offset * 3.14) * waveAmplitude * 0.6;
        float wave3 = cos(p.x * 1.5 + t * 1.3 + offset * 4.71) * waveAmplitude * 0.4;
        
        float y = baseY + wave1 + wave2 + wave3;
        
        // Distance à la ligne
        float dist = abs(p.y - y);
        
        // Créer la ligne avec glow
        float line = smoothstep(lineThickness * 2.0, 0.0, dist);
        float glow = exp(-dist * 15.0) * glowIntensity * 0.3;
        
        // Couleurs qui varient selon la position et le temps
        float hue = offset + t * 0.1 + p.x * 0.2;
        
        // Utiliser les couleurs personnalisables
        vec3 lineColor = mix(color1.rgb, color2.rgb, sin(hue * 6.28) * 0.5 + 0.5);
        lineColor = mix(lineColor, color3.rgb, cos(hue * 3.14) * colorMix);
        
        // Ajouter variation de luminosité le long de la ligne
        float brightness = 0.7 + 0.3 * sin(p.x * 4.0 + t + offset * 2.0);
        lineColor *= brightness;
        
        // Accumuler la couleur
        col += lineColor * (line + glow);
    }
    
    // Ajouter un léger gradient de fond
    vec3 bg = mix(vec3(0.02, 0.1, 0.15), vec3(0.0, 0.15, 0.2), uv.y);
    col += bg * 0.3;
    
    // Post-processing pour un look plus doux
    col = pow(col, vec3(0.9));
    
    gl_FragColor = vec4(col, 1.0);
}