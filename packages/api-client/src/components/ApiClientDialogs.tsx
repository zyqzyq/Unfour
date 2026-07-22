import type {
  ApiCollection,
  ApiCollectionFolder,
  ApiSavedRequest,
} from "@unfour/command-client";
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
  savedRequests,
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
  savedRequests: ApiSavedRequest[];
  saveDialogTab: ApiRequestTab | null;
}) {
  return (
    <>
      {saveDialogTab && (
        <ApiSaveDialog
          collections={collections}
          defaultCollectionId={saveDialogTab.draft.collectionId}
          defaultParentFolderId={saveDialogTab.draft.parentFolderId}
          defaultName={saveDialogTab.draft.name}
          folders={folders}
          key={saveDialogTab.id}
          savedRequests={savedRequests}
          onCancel={onCancelSave}
          onSave={onSaveIdentity}
          open
          saving={saveDialogTab.saving}
        />
      )}
      <ApiCloseRequestDialog
        onCancel={onCancelClose}
        onDiscard={onDiscardClose}
        onSave={onSaveClose}
        open={Boolean(closeDialogTab)}
        title={closeDialogTab ? requestTabTitle(closeDialogTab) : ""}
      />
    </>
  );
}
