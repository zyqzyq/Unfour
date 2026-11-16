import { useCallback } from "react";
import { useFeedbackErrorHandler } from "@unfour/ui";

/** Explicit copy actions share failure feedback, including unavailable APIs. */
export function useSshClipboardWrite() {
  const handleError = useFeedbackErrorHandler();
  return useCallback(async (text: string): Promise<boolean> => {
    try {
      if (!navigator.clipboard?.writeText) throw new DOMException("", "NotSupportedError");
      await navigator.clipboard.writeText(text);
      return true;
    } catch (error) {
      handleError(error, { key: "feedback.ssh.clipboardWriteFailed" });
      return false;
    }
  }, [handleError]);
}
