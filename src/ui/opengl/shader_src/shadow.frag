#version 330 core

out vec4 out_color;

uniform sampler2D tex;

in vec2 tex_coord;

void main() {
	out_color = vec4(0.0, 0.0, 0.0, texture(tex, tex_coord).r);
}

