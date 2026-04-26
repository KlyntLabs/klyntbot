export interface DetectedPattern {
  name: string;
  confidence: number;
  signalCount: number;
  description: string;
  domain: string;
}

export interface InterventionLog {
  id: string;
  interventionType: string;
  message: string;
  triggerName: string;
  feedback: string | null;
  deliveredAt: string;
  feedbackAt: string | null;
}
