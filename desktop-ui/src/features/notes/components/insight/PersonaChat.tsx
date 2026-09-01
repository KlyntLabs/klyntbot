import { useInsightChat } from "../../hooks/useInsightChat";
import { InsightChatInput } from "./InsightChatInput";

interface PersonaChatProps {
  noteId: string;
  personaId: string;
  personaName: string;
}

export function PersonaChat({ noteId, personaId, personaName }: PersonaChatProps) {
  const chat = useInsightChat(noteId, `persona:${personaId}`, true);

  return (
    <InsightChatInput {...chat} placeholder={`Ask ${personaName}...`} speakerLabel={personaName} />
  );
}
