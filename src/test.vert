#version 140

uniform float time;

in vec2 position;

out vec2 fPosition;

void main() {
	gl_Position = vec4(position, 0.0, 1.0);
	fPosition = position;
}
