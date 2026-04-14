import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

/** Shared wiring for a sortable entity card/row — ref + drag style + listeners. */
export function useSortableEntity(id: string) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id,
  });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.4 : 1,
  };
  return { setNodeRef, style, dragProps: { ...attributes, ...listeners } };
}
