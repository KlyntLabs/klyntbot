import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface PracticeSegment {
  index: number;
  text: string;
  segmentType: string;
  suggestedFocus: string;
  skipped: boolean;
}

export interface PracticeSegmentResponse {
  segments: PracticeSegment[];
  estimatedMins: number;
  cachedAt: string | null;
}

export function useSmartSegmentation() {
  const [segments, setSegments] = useState<PracticeSegment[]>([]);
  const [estimatedMins, setEstimatedMins] = useState<number>(0);
  const [cachedAt, setCachedAt] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const segment = useCallback(async (noteId: string, sourceLang: string, targetLang: string) => {
    setLoading(true);
    setError(null);
    try {
      const response = await ipc<PracticeSegmentResponse>("practice_segment_note", {
        params: { noteId, sourceLang, targetLang },
      });
      setSegments(response.segments);
      setEstimatedMins(response.estimatedMins);
      setCachedAt(response.cachedAt);
      return response;
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : "Segmentation failed";
      setError(msg);
      return null;
    } finally {
      setLoading(false);
    }
  }, []);

  return { segments, estimatedMins, cachedAt, loading, error, segment };
}
