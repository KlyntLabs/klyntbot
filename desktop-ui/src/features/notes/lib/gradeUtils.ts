/** Canonical grade-to-number mapping for practice sessions. 0-100 scale. */
export function gradeToNumber(grade: string): number {
  const map: Record<string, number> = {
    "A+": 100,
    A: 95,
    "A-": 90,
    "B+": 87,
    B: 83,
    "B-": 80,
    "C+": 77,
    C: 73,
    "C-": 70,
    "D+": 67,
    D: 63,
    "D-": 60,
    F: 40,
  };
  return map[grade] ?? 50;
}

/** Convert a numeric score (0-100) back to a letter grade. */
export function numberToGrade(score: number): string {
  if (score >= 97) return "A+";
  if (score >= 93) return "A";
  if (score >= 90) return "A-";
  if (score >= 87) return "B+";
  if (score >= 83) return "B";
  if (score >= 80) return "B-";
  if (score >= 77) return "C+";
  if (score >= 73) return "C";
  if (score >= 70) return "C-";
  if (score >= 67) return "D+";
  if (score >= 63) return "D";
  if (score >= 60) return "D-";
  return "F";
}

/** Whether a grade is "strong" (A+ or A only). Weak = A- and below. */
export function isStrongGrade(grade: string): boolean {
  return grade === "A+" || grade === "A";
}

/** Tailwind text color class for a letter grade. */
export function gradeColorClass(grade: string): string {
  if (grade.startsWith("A")) return "text-green-400";
  if (grade.startsWith("B")) return "text-yellow-400";
  if (grade.startsWith("C")) return "text-orange-400";
  return "text-red-400";
}

/** Tailwind background color class for a letter grade (at 15% opacity). */
export function gradeBgClass(grade: string): string {
  if (grade.startsWith("A")) return "bg-green-400/15";
  if (grade.startsWith("B")) return "bg-yellow-400/15";
  if (grade.startsWith("C")) return "bg-orange-400/15";
  return "bg-red-400/15";
}
