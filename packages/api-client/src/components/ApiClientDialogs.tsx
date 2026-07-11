import type {
  ApiCollection,
  ApiCollectionFolder,
  ApiSavedRequest,
} from "@unfour/command-client";
import { ConfirmDialog, useI18n } from "@unfour/ui";

import { requestTabTitle, type ApiRequestTab } from "../model/request-tabs";
import { ApiCloseRequestDialog } from "./ApiCloseRequestDialog";
import { ApiSaveDialog, type SaveIdentity } from "./ApiSaveDialog";

export function ApiClientDialogs({
  closeDialogTab,
  collections,
  environmentCloseDialogOpen,
  folders,
  onCancelClose,
  onCancelSave,
  onCloseEnvironment,
  onDiscardClose,
  onEnvironmentDialogOpenChange,
  onSaveClose,
  onSaveIdentity,
  savedRequests,
  saveDialogTab,
}: {
  closeDialogTab: ApiRequestTab | null;
  collections: ApiCollection[];
  environmentCloseDialogOpen: boolean;
  folders: ApiCollectionFolder[];
  onCancelClose: () => void;
  onCancelSave: () => void;
  onCloseEnvironment: () => void;
  onDiscardClose: () => void;
  onEnvironmentDialogOpenChange: (open: boolean) => void;
  onSaveClose: () => void;
  onSaveIdentity: (identity: SaveIdentity) => void;
  savedRequests: ApiSavedRequest[];
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
      <ConfirmDialog
        confirmLabel={t("api.environment.discard")}
        description={t("api.environment.discardChangesDescription")}
        onConfirm={onCloseEnvironment}
        onOpenChange={onEnvironmentDialogOpenChange}
        open={environmentCloseDialogOpen}
        title={t("api.environment.discardChangesTitle")}
      />
    </>
  );
}
