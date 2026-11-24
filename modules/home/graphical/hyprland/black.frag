#version 300 es
// Hyprland screen shader that draws everything black: the "blank" idle stage
// (hypridle.nix). The monitors keep their signal, so waking is instant.
precision mediump float;
in vec2 v_texcoord;
uniform sampler2D tex;
out vec4 fragColor;
void main() {
    // Sample tex anyway so the uniform isn't optimised out
    fragColor = vec4(texture(tex, v_texcoord).rgb * 0.0, 1.0);
}
