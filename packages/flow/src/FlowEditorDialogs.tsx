import type { FlowDefinition } from "@unfour/command-client";
import { ConfirmDialog, useI18n } from "@unfour/ui";
import { START } from "./canvasGraph";
import type { FlowEditor } from "./useFlowEditor";

export function FlowEditorDialogs({ editor, resetSelection }: {
  editor: FlowEditor; resetSelection: (flow: FlowDefinition) => void;
}) {
  const { t } = useI18n();
  const { confirm, setConfirm, busy, removingNode, setRemovingNode, impact, pendingSelection, setPendingSelection } = editor;
  return (
    <>
      <ConfirmDialog
        open={confirm !== null}
        onOpenChange={(open) => {
          if (!open) setConfirm(null);
        }}
        title={t("flow.delete")}
        description={t("flow.deleteHelp")}
        confirmLabel={t("flow.delete")}
        pending={busy}
        onConfirm={() => void editor.deleteDefinition()}
      />
      <ConfirmDialog open={removingNode !== null} onOpenChange={(open) => { if (!open) setRemovingNode(null); }} title={t("flow.canvas.remove")} description={<span className="grid gap-2"><span>{t("flow.canvas.removeHelp")}</span><span>{t("flow.canvas.incoming")}: {impact?.incoming.map((name) => name === START ? t("flow.canvas.start") : name).join(", ") || t("flow.none")}</span>{Boolean(impact?.references.length) && <span role="alert">{t("flow.canvas.referenced")}: {impact?.references.join(", ")}</span>}</span>} confirmLabel={t(impact?.references.length ? "flow.acknowledge" : "flow.remove")} onConfirm={editor.removeStep} />
      <ConfirmDialog
        open={pendingSelection !== null}
        onOpenChange={(open) => {
          if (!open) setPendingSelection(null);
        }}
        title={t("flow.unsaved")}
        description={t("flow.discardHelp")}
        confirmLabel={t("flow.discard")}
        onConfirm={() => editor.discardSelection(resetSelection)}
      />
    </>
  );
}
