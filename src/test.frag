#version 140

uniform float time;

in vec2 fPosition;

out vec4 color;

void main() {
	color = vec4(fPosition.x, (1.0 + sin(time)) / 2.0, 1.0, 1.0);
}
