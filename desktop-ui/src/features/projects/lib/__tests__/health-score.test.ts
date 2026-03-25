import { computeHealthScore, type HealthScoreInput } from "../health-score";

describe("computeHealthScore", () => {
  it("computes weighted score from all factors", () => {
    const input: HealthScoreInput = {
      okrProgress: 0.8,
      taskVelocity: 0.6,
      insightFreshness: 1.0,
      focusQuality: 0.9,
    };
    // 0.8*0.60 + 0.6*0.20 + 1.0*0.10 + 0.9*0.10 = 0.48 + 0.12 + 0.10 + 0.09 = 0.79
    const result = computeHealthScore(input);
    expect(result.score).toBeCloseTo(79, 0);
    expect(result.color).toBe("green");
    expect(result.breakdown).toHaveLength(4);
  });

  it("returns yellow for mid-range scores", () => {
    const input: HealthScoreInput = {
      okrProgress: 0.5,
      taskVelocity: 0.4,
      insightFreshness: 0.5,
      focusQuality: 0.3,
    };
    const result = computeHealthScore(input);
    expect(result.color).toBe("yellow");
  });

  it("returns red for low scores", () => {
    const input: HealthScoreInput = {
      okrProgress: 0.2,
      taskVelocity: 0.1,
      insightFreshness: 0.0,
      focusQuality: 0.1,
    };
    const result = computeHealthScore(input);
    expect(result.color).toBe("red");
  });

  it("clamps all inputs to 0-1 range", () => {
    const input: HealthScoreInput = {
      okrProgress: 1.5,
      taskVelocity: -0.2,
      insightFreshness: 2.0,
      focusQuality: 1.0,
    };
    const result = computeHealthScore(input);
    expect(result.score).toBeLessThanOrEqual(100);
    expect(result.score).toBeGreaterThanOrEqual(0);
  });
});
