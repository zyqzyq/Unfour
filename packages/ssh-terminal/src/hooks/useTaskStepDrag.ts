import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import type { SshTaskSaveInput } from "@unfour/command-client";
import { reorderTaskStep } from "../model/task-template";

export function useTaskStepDrag(
  draft: SshTaskSaveInput,
  onChange: (draft: SshTaskSaveInput) => void,
  setExpandedIndex: (index: number) => void,
) {
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [overIndex, setOverIndex] = useState<number | null>(null);
  const dragFromRef = useRef<number | null>(null);
  const overIndexRef = useRef<number | null>(null);
  const draftRef = useRef({ draft, onChange });
  const dragCleanupRef = useRef<(() => void) | null>(null);
  useLayoutEffect(() => { draftRef.current = { draft, onChange }; }, [draft, onChange]);
  useEffect(() => () => dragCleanupRef.current?.(), []);

  useEffect(() => {
    overIndexRef.current = overIndex;
  }, [overIndex]);

  function finishStepDrag() {
    const from = dragFromRef.current;
    const to = overIndexRef.current;
    dragFromRef.current = null;
    const { draft: current, onChange: changeDraft } = draftRef.current;
    if (from !== null && to !== null && from !== to) {
      changeDraft({
        ...current,
        steps: reorderTaskStep(current.steps, from, to),
      });
      setExpandedIndex(to);
    }
    setDragIndex(null);
    setOverIndex(null);
  }

  function onStepDragHandlePointerDown(
    index: number,
    event: ReactPointerEvent<HTMLSpanElement>,
  ) {
    if (event.button !== 0) return;
    event.preventDefault();
    dragCleanupRef.current?.();
    dragFromRef.current = index;
    overIndexRef.current = index;
    setDragIndex(index);
    setOverIndex(index);
    const target = event.currentTarget;
    target.setPointerCapture(event.pointerId);

    function onMove(moveEvent: PointerEvent) {
      const el = document.elementFromPoint(moveEvent.clientX, moveEvent.clientY);
      const row = el?.closest("[data-step-index]");
      if (!row) return;
      const next = Number(row.getAttribute("data-step-index"));
      if (Number.isNaN(next)) return;
      overIndexRef.current = next;
      setOverIndex(next);
    }

    function onUp() {
      target.releasePointerCapture(event.pointerId);
      cleanup();
      finishStepDrag();
    }

    function cleanup() {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      dragCleanupRef.current = null;
    }

    dragCleanupRef.current = cleanup;
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  return { dragIndex, overIndex, onStepDragHandlePointerDown };
}
