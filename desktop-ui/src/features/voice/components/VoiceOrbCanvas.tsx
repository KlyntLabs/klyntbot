import type { ConversationPhase } from "@features/voice/hooks/useVoiceConversation";
import { useEffect, useRef } from "react";

// ─── Phase → uniform mapping ────────────────────────────────────────────────

const PHASE_MAP: Record<ConversationPhase, number> = {
  idle: 0,
  listening: 1,
  reflecting: 2,
  speaking: 3,
};

// ─── Vertex shader ──────────────────────────────────────────────────────────

const VERTEX_SRC = /* glsl */ `#version 300 es
precision highp float;
in vec2 a_position;
void main() {
  gl_Position = vec4(a_position, 0.0, 1.0);
}
`;

// ─── Fragment shader ────────────────────────────────────────────────────────

const FRAGMENT_SRC = /* glsl */ `#version 300 es
precision highp float;

uniform float u_time;
uniform float u_phase;
uniform float u_rms;
uniform vec2  u_resolution;
uniform float u_transition;

out vec4 fragColor;

// ── Hash & noise ────────────────────────────────────────────────────────────

vec3 hash3(vec3 p) {
  p = vec3(
    dot(p, vec3(127.1, 311.7, 74.7)),
    dot(p, vec3(269.5, 183.3, 246.1)),
    dot(p, vec3(113.5, 271.9, 124.6))
  );
  return fract(sin(p) * 43758.5453123);
}

float noise3(vec3 p) {
  vec3 i = floor(p);
  vec3 f = fract(p);
  vec3 u = f * f * (3.0 - 2.0 * f);

  float a = dot(hash3(i + vec3(0,0,0)), f - vec3(0,0,0));
  float b = dot(hash3(i + vec3(1,0,0)), f - vec3(1,0,0));
  float c = dot(hash3(i + vec3(0,1,0)), f - vec3(0,1,0));
  float d = dot(hash3(i + vec3(1,1,0)), f - vec3(1,1,0));
  float e = dot(hash3(i + vec3(0,0,1)), f - vec3(0,0,1));
  float g = dot(hash3(i + vec3(1,0,1)), f - vec3(1,0,1));
  float h = dot(hash3(i + vec3(0,1,1)), f - vec3(0,1,1));
  float k = dot(hash3(i + vec3(1,1,1)), f - vec3(1,1,1));

  return mix(
    mix(mix(a, b, u.x), mix(c, d, u.x), u.y),
    mix(mix(e, g, u.x), mix(h, k, u.x), u.y),
    u.z
  );
}

float fbm(vec3 p) {
  float v = 0.0;
  float a = 0.5;
  vec3 shift = vec3(100.0);
  for (int i = 0; i < 4; i++) {
    v += a * noise3(p);
    p = p * 2.0 + shift;
    a *= 0.5;
  }
  return v;
}

// ── Phase color palettes ────────────────────────────────────────────────────

vec3 phaseColor(float ph, float t, float n) {
  // Idle: soft cyan-teal
  vec3 idle1 = vec3(0.2, 0.7, 0.75);
  vec3 idle2 = vec3(0.1, 0.5, 0.6);
  vec3 cIdle = mix(idle1, idle2, 0.5 + 0.5 * sin(t * 0.5 + n * 3.0));

  // Listening: bright cyan
  vec3 listen1 = vec3(0.1, 0.85, 0.95);
  vec3 listen2 = vec3(0.3, 0.65, 1.0);
  vec3 cListen = mix(listen1, listen2, 0.5 + 0.5 * sin(t * 1.5 + n * 4.0));

  // Reflecting: warm amber-orange
  vec3 reflect1 = vec3(0.95, 0.65, 0.15);
  vec3 reflect2 = vec3(0.85, 0.4, 0.1);
  vec3 cReflect = mix(reflect1, reflect2, 0.5 + 0.5 * sin(t * 0.8 + n * 2.5));

  // Speaking: soft green
  vec3 speak1 = vec3(0.3, 0.85, 0.5);
  vec3 speak2 = vec3(0.15, 0.7, 0.4);
  vec3 cSpeak = mix(speak1, speak2, 0.5 + 0.5 * sin(t * 1.2 + n * 3.5));

  // Smooth blending across phases
  float w0 = max(0.0, 1.0 - abs(ph - 0.0));
  float w1 = max(0.0, 1.0 - abs(ph - 1.0));
  float w2 = max(0.0, 1.0 - abs(ph - 2.0));
  float w3 = max(0.0, 1.0 - abs(ph - 3.0));
  float wSum = w0 + w1 + w2 + w3;

  return (cIdle * w0 + cListen * w1 + cReflect * w2 + cSpeak * w3) / wSum;
}

// ── Main ────────────────────────────────────────────────────────────────────

void main() {
  vec2 uv = gl_FragCoord.xy / u_resolution;
  vec2 centered = (uv - 0.5) * 2.0;
  // Correct aspect ratio
  centered.x *= u_resolution.x / u_resolution.y;

  float dist = length(centered);
  float angle = atan(centered.y, centered.x);

  float t = u_time;
  float ph = u_phase;
  float rms = u_rms;

  // ── Breathing pulse (all phases, stronger in idle) ──
  float breathSpeed = mix(0.8, 1.5, smoothstep(0.0, 1.0, ph));
  float breath = 0.02 * sin(t * breathSpeed) * mix(1.0, 0.4, smoothstep(0.0, 1.0, ph));

  // ── Base sphere radius with audio reactivity ──
  float baseRadius = 0.45 + breath;
  float audioPulse = rms * 0.12 * smoothstep(0.5, 1.5, abs(ph - 0.5) + abs(ph - 2.5));
  baseRadius += audioPulse;

  // ── Phase weights (used for noise, color, and effect blending) ──
  float w0 = max(0.0, 1.0 - abs(ph - 0.0));
  float w1 = max(0.0, 1.0 - abs(ph - 1.0));
  float w2 = max(0.0, 1.0 - abs(ph - 2.0));
  float w3 = max(0.0, 1.0 - abs(ph - 3.0));
  float wSum = w0 + w1 + w2 + w3;

  // ── Noise displacement for organic surface ──
  vec3 noiseCoord = vec3(centered * 2.5, t * 0.3);
  float noiseScale = 0.08;
  float spiralAngle = angle + dist * 4.0 - t * 1.5;

  // Guarded by weight to skip inactive phase noise (75% GPU savings in steady state)
  float idleNoise = 0.0;
  if (w0 > 0.01) idleNoise = fbm(noiseCoord + vec3(t * 0.15, t * 0.1, 0.0));

  float listenNoise = 0.0;
  if (w1 > 0.01) listenNoise = fbm(noiseCoord * 1.3 + vec3(0.0, 0.0, t * 0.5))
    + 0.06 * sin(dist * 12.0 - t * 4.0) * rms;

  float reflectNoise = 0.0;
  if (w2 > 0.01) reflectNoise = fbm(
    noiseCoord + vec3(cos(spiralAngle), sin(spiralAngle), t * 0.2) * 0.6
  );

  float speakNoise = 0.0;
  if (w3 > 0.01) speakNoise = fbm(noiseCoord * 0.9 + vec3(0.0, 0.0, t * 0.25));

  float blendedNoise = (idleNoise * w0 + listenNoise * w1
    + reflectNoise * w2 + speakNoise * w3) / wSum;

  float displacement = blendedNoise * noiseScale;
  float sphereDist = dist - baseRadius - displacement;

  // ── Sphere body ──
  float sphere = 1.0 - smoothstep(-0.03, 0.03, sphereDist);

  // ── Inner glow (exponential falloff from center) ──
  float innerGlow = exp(-dist * 3.0) * 0.6;

  // ── Outer halo (exponential falloff from sphere edge) ──
  float outerHalo = exp(-max(0.0, sphereDist) * 6.0) * 0.35;
  // Strengthen halo during listening/speaking with RMS
  outerHalo += exp(-max(0.0, sphereDist) * 4.0) * rms * 0.2
    * (w1 + w3) / max(w1 + w3 + 0.01, 1.0);

  // ── Speaking: concentric wave rings ──
  float waveRings = 0.0;
  if (w3 > 0.01) {
    for (int i = 0; i < 3; i++) {
      float ringOffset = float(i) * 2.094; // 2pi/3
      float ringDist = dist - baseRadius;
      float wave = sin(ringDist * 18.0 - t * 3.5 + ringOffset);
      wave = smoothstep(0.7, 1.0, wave);
      float ringFade = exp(-max(0.0, ringDist) * 3.0);
      waveRings += wave * ringFade * 0.15 * (1.0 + rms * 0.5);
    }
    waveRings *= w3 / wSum;
  }

  // ── Reflecting: spiral pattern enhancement ──
  float spiralPattern = 0.0;
  if (w2 > 0.01) {
    float sp = sin(angle * 3.0 + dist * 8.0 - t * 2.0);
    sp = smoothstep(0.3, 0.9, sp);
    float spFade = exp(-abs(sphereDist) * 5.0);
    spiralPattern = sp * spFade * 0.12 * w2 / wSum;
  }

  // ── Compose color ──
  vec3 col = phaseColor(ph, t, blendedNoise);
  float alpha = sphere * 0.85 + innerGlow + outerHalo + waveRings + spiralPattern;

  // ── Vignette ──
  float vignette = 1.0 - smoothstep(0.6, 1.2, dist);
  alpha *= vignette;

  alpha = clamp(alpha, 0.0, 1.0);

  // Premultiplied alpha output
  fragColor = vec4(col * alpha, alpha);
}
`;

// ─── WebGL helpers ──────────────────────────────────────────────────────────

function compileShader(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("Failed to create shader");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const info = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`Shader compile error: ${info}`);
  }
  return shader;
}

function createProgram(gl: WebGL2RenderingContext, vertSrc: string, fragSrc: string): WebGLProgram {
  const vert = compileShader(gl, gl.VERTEX_SHADER, vertSrc);
  const frag = compileShader(gl, gl.FRAGMENT_SHADER, fragSrc);
  const program = gl.createProgram();
  if (!program) throw new Error("Failed to create program");
  gl.attachShader(program, vert);
  gl.attachShader(program, frag);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const info = gl.getProgramInfoLog(program);
    gl.deleteProgram(program);
    gl.deleteShader(vert);
    gl.deleteShader(frag);
    throw new Error(`Program link error: ${info}`);
  }
  // Shaders can be flagged for deletion after linking
  gl.deleteShader(vert);
  gl.deleteShader(frag);
  return program;
}

// ─── Props ──────────────────────────────────────────────────────────────────

interface Props {
  phase: ConversationPhase;
  audioLevel: number;
}

// ─── Component ──────────────────────────────────────────────────────────────

export function VoiceOrbCanvas({ phase, audioLevel }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const audioLevelRef = useRef(audioLevel);
  const phaseRef = useRef(phase);
  const rafRef = useRef(0);

  // Update audioLevel ref without re-triggering the RAF effect
  useEffect(() => {
    audioLevelRef.current = audioLevel;
  }, [audioLevel]);

  // Update phase ref without re-triggering the RAF effect
  useEffect(() => {
    phaseRef.current = phase;
  }, [phase]);

  // Main WebGL setup & render loop — runs once on mount
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const gl = canvas.getContext("webgl2", {
      alpha: true,
      premultipliedAlpha: false,
    });
    if (!gl) {
      console.error("VoiceOrbCanvas: WebGL2 not supported");
      return;
    }
    // Alias gl.useProgram to avoid Biome misinterpreting it as a React hook
    const activateProgram = gl.useProgram.bind(gl);

    // Create shader program
    const program = createProgram(gl, VERTEX_SRC, FRAGMENT_SRC);

    // Fullscreen quad (-1 to 1)
    const positions = new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]);
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);

    // VAO
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const aPos = gl.getAttribLocation(program, "a_position");
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);

    // Uniform locations
    activateProgram(program);
    const uTime = gl.getUniformLocation(program, "u_time");
    const uPhase = gl.getUniformLocation(program, "u_phase");
    const uRms = gl.getUniformLocation(program, "u_rms");
    const uResolution = gl.getUniformLocation(program, "u_resolution");
    const uTransition = gl.getUniformLocation(program, "u_transition");

    // Enable blending for transparency
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

    // Animation state
    const startTime = performance.now();
    let currentPhase = PHASE_MAP[phaseRef.current];
    let targetPhase = currentPhase;
    let smoothRms = 0;
    let transitionProgress = 1.0;
    let lastTransitionStart = 0;
    const TRANSITION_DURATION = 300; // ms

    const render = (now: number) => {
      const elapsed = (now - startTime) / 1000;

      // Resize canvas to match display size * DPR
      const dpr = window.devicePixelRatio || 1;
      const displayWidth = canvas.clientWidth;
      const displayHeight = canvas.clientHeight;
      const pixelWidth = Math.round(displayWidth * dpr);
      const pixelHeight = Math.round(displayHeight * dpr);

      if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
        canvas.width = pixelWidth;
        canvas.height = pixelHeight;
        gl.viewport(0, 0, pixelWidth, pixelHeight);
      }

      // Phase interpolation (300ms lerp)
      const newTarget = PHASE_MAP[phaseRef.current];
      if (newTarget !== targetPhase) {
        targetPhase = newTarget;
        lastTransitionStart = now;
      }

      const transitionElapsed = now - lastTransitionStart;
      transitionProgress = Math.min(transitionElapsed / TRANSITION_DURATION, 1.0);
      // Ease-in-out quadratic
      const eased =
        transitionProgress < 0.5
          ? 2 * transitionProgress * transitionProgress
          : 1 - (-2 * transitionProgress + 2) ** 2 / 2;
      currentPhase += (targetPhase - currentPhase) * eased;

      // RMS smoothing
      const targetRms = audioLevelRef.current;
      smoothRms += (targetRms - smoothRms) * 0.15;

      // Clear
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);

      // Draw
      activateProgram(program);
      gl.uniform1f(uTime, elapsed);
      gl.uniform1f(uPhase, currentPhase);
      gl.uniform1f(uRms, smoothRms);
      gl.uniform2f(uResolution, pixelWidth, pixelHeight);
      gl.uniform1f(uTransition, transitionProgress);

      gl.bindVertexArray(vao);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      gl.bindVertexArray(null);

      rafRef.current = requestAnimationFrame(render);
    };

    rafRef.current = requestAnimationFrame(render);

    // Cleanup on unmount
    return () => {
      cancelAnimationFrame(rafRef.current);
      gl.deleteProgram(program);
      gl.deleteBuffer(buffer);
      gl.deleteVertexArray(vao);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      style={{
        width: "100%",
        height: "100%",
        display: "block",
      }}
    />
  );
}
