import type {
  ApiCollection,
  ApiCollectionFolder,
  ApiSavedRequest,
} from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { requestTabTitle, type ApiRequestTab } from "../model/request-tabs";
import { ApiCloseRequestDialog } from "./ApiCloseRequestDialog";
import { ApiSaveDialog, type SaveIdentity } from "./ApiSaveDialog";

export function ApiClientDialogs({
  closeDialogTab,
  collections,
  folders,
  onCancelClose,
  onCancelSave,
  onDiscardClose,
  onSaveClose,
  onSaveIdentity,
  onRestoreFocus,
  savedRequests,
  saveDialogError,
  saveDialogPending,
  saveDialogTab,
}: {
  closeDialogTab: ApiRequestTab | null;
  collections: ApiCollection[];
  folders: ApiCollectionFolder[];
  onCancelClose: () => void;
  onCancelSave: () => void;
  onDiscardClose: () => void;
  onSaveClose: () => void;
  onSaveIdentity: (identity: SaveIdentity) => void;
  onRestoreFocus: () => void;
  savedRequests: ApiSavedRequest[];
  saveDialogError: string | null;
  saveDialogPending?: boolean;
  saveDialogTab: ApiRequestTab | null;
}) {
  const { t } = useI18n();

  return (
    <>
      {saveDialogTab && (
        <ApiSaveDialog
          collections={collections}
          defaultCollectionId={saveDialogTab.draft.collectionId}
          defaultParentFolderId={saveDialogTab.draft.parentFolderId}
          defaultName={saveDialogTab.draft.name}
          error={saveDialogError}
          notice={saveDialogTab.source === "history" ? t("api.history.snapshotNotice") : undefined}
          folders={folders}
          key={saveDialogTab.id}
          savedRequests={savedRequests}
          onCancel={onCancelSave}
          onSave={onSaveIdentity}
          open
          saving={Boolean(saveDialogPending) || saveDialogTab.saving}
        />
      )}
      <ApiCloseRequestDialog
        onRestoreFocus={onRestoreFocus}
        onCancel={onCancelClose}
        onDiscard={onDiscardClose}
        onSave={onSaveClose}
        open={Boolean(closeDialogTab)}
        title={
          closeDialogTab
            ? requestTabTitle(closeDialogTab, t("api.request.untitled"))
            : ""
        }
      />
    </>
  );
}
