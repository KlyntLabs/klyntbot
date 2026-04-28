import { useMemo, useState } from "react";
import { ChevronDown, ChevronRight, AlertCircle } from "lucide-react";
import { isErrorEvent } from "./eventHelpers";
import type { WireEventDto } from "./types";

interface StepNode {
  eventIndex: number;
  stepNumber: number;
  hasError: boolean;
}

interface TurnNode {
  eventIndex: number;
  turnNumber: number;
  userInput: string;
  steps: StepNode[];
  hasError: boolean;
}

function buildTree(events: WireEventDto[]): TurnNode[] {
  const turns: TurnNode[] = [];
  let currentTurn: TurnNode | null = null;
  let currentStep: StepNode | null = null;

  events.forEach((event, idx) => {
    if (event.kind === "turnBegin") {
      const p = event.payloadDecoded as any;
      let input = "";
      if (typeof p?.user_input === "string") {
        input = p.user_input.slice(0, 60);
      } else if (Array.isArray(p?.user_input) && p.user_input.length > 0) {
        input = String(p.user_input[0].text ?? "").slice(0, 60);
      }
      currentTurn = {
        eventIndex: idx,
        turnNumber: turns.length + 1,
        userInput: input,
        steps: [],
        hasError: false,
      };
      turns.push(currentTurn);
      currentStep = null;
    } else if (event.kind === "stepBegin" && currentTurn) {
      currentStep = {
        eventIndex: idx,
        stepNumber: currentTurn.steps.length + 1,
        hasError: false,
      };
      currentTurn.steps.push(currentStep);
    } else if (isErrorEvent(event)) {
      if (currentStep) currentStep.hasError = true;
      if (currentTurn) currentTurn.hasError = true;
    }
  });

  return turns;
}

interface TurnTreeProps {
  events: WireEventDto[];
  onScrollToIndex: (eventIndex: number) => void;
}

export function TurnTree({ events, onScrollToIndex }: TurnTreeProps) {
  const tree = useMemo(() => buildTree(events), [events]);
  const [collapsedTurns, setCollapsedTurns] = useState<Set<number>>(new Set());

  if (tree.length === 0) return null;

  return (
    <nav className="cm-turn-tree" aria-label="Turn navigation">
      {tree.map((turn) => {
        const collapsed = collapsedTurns.has(turn.eventIndex);
        return (
          <div key={turn.eventIndex} className="cm-turn-tree__node">
            <button
              type="button"
              className={
                "cm-turn-tree__turn" +
                (turn.hasError ? " cm-turn-tree__turn--error" : "")
              }
              onClick={() => {
                setCollapsedTurns((prev) => {
                  const next = new Set(prev);
                  if (next.has(turn.eventIndex)) next.delete(turn.eventIndex);
                  else next.add(turn.eventIndex);
                  return next;
                });
              }}
            >
              {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
              <span>Turn {turn.turnNumber}</span>
              {turn.hasError && <AlertCircle size={10} className="cm-turn-tree__error-icon" />}
            </button>
            {!collapsed && (
              <div className="cm-turn-tree__steps">
                {turn.steps.map((step) => (
                  <button
                    key={step.eventIndex}
                    type="button"
                    className={
                      "cm-turn-tree__step" +
                      (step.hasError ? " cm-turn-tree__step--error" : "")
                    }
                    onClick={() => onScrollToIndex(step.eventIndex)}
                  >
                    Step {step.stepNumber}
                    {step.hasError && <AlertCircle size={10} className="cm-turn-tree__error-icon" />}
                  </button>
                ))}
              </div>
            )}
          </div>
        );
      })}
    </nav>
  );
}
