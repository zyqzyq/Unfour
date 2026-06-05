import { SplitPane } from "@unfour/ui";
import type { ApiHistoryItem } from "@unfour/command-client";
import { useApiHistory } from "./hooks/useApiHistory";
import { useApiLayout } from "./hooks/useApiLayout";
import { useApiRequest } from "./hooks/useApiRequest";
import { ApiRequestEditor } from "./components/ApiRequestEditor";
import { ApiRequestToolbar } from "./components/ApiRequestToolbar";
import { ApiResponseViewer } from "./components/ApiResponseViewer";

export function ApiDebuggerPage({
  selectedRequestId,
  setSelectedRequestId,
  workspaceId,
}: {
  selectedRequestId: string | null;
  setSelectedRequestId: (requestId: string | null) => void;
  workspaceId: string;
}) {
  const layout = useApiLayout();
  const request = useApiRequest({
    selectedRequestId,
    setSelectedRequestId,
    workspaceId,
  });
  const history = useApiHistory({
    onReplayLoaded: (detail) => {
      setSelectedRequestId(null);
      request.loadHistoryRequest(detail);
      layout.setResultTab("response");
    },
    workspaceId,
  });

  const title =
    request.selectedSavedRequest?.name ??
    (selectedRequestId ? "Request not found" : "New request");

  return (
    <form
      className="flex h-full min-h-0 flex-col bg-[var(--u-color-surface)]"
      onSubmit={request.submit}
    >
      <input
        accept="application/json"
        className="sr-only"
        disabled={request.importCollectionMutation.isPending}
        onChange={(event) => {
          void request.importCollection(event.target.files?.[0]);
          event.target.value = "";
        }}
        ref={request.importInputRef}
        type="file"
      />
      <ApiRequestToolbar
        canDelete={Boolean(selectedRequestId)}
        canDuplicate={Boolean(selectedRequestId)}
        canExport={request.savedRequests.length > 0}
        collectionStatus={request.collectionStatus}
        deleting={request.deleteSavedMutation.isPending}
        duplicating={request.duplicateSavedMutation.isPending}
        importing={request.importCollectionMutation.isPending}
        input={request.input}
        onDelete={request.deleteSelectedRequest}
        onDuplicate={request.duplicateSelectedRequest}
        onExport={request.exportCollection}
        onImport={() => request.importInputRef.current?.click()}
        onNewRequest={request.newRequest}
        onSave={() => request.saveMutation.mutate(request.input)}
        requestState={request.requestState}
        saving={request.saveMutation.isPending}
        selectedUrl={request.selectedSavedRequest?.url ?? ""}
        sending={request.sendMutation.isPending}
        title={title}
      />
      <SplitPane className="min-h-0 flex-1 flex-col xl:flex-row">
        <ApiRequestEditor
          body={request.body}
          envVariables={request.envVariables}
          folderPath={request.folderPath}
          headers={request.headers}
          method={request.method}
          name={request.name}
          onBodyChange={request.setBody}
          onEnvVariablesChange={request.setEnvVariables}
          onFolderPathChange={request.setFolderPath}
          onHeadersChange={request.setHeaders}
          onMethodChange={request.setMethod}
          onNameChange={request.setName}
          onQueryChange={request.setQuery}
          onSaveEnvironment={request.saveEnvironment}
          onTabChange={layout.setRequestTab}
          onUrlChange={request.setUrl}
          query={request.query}
          savingEnvironment={request.saveEnvironmentMutation.isPending}
          tab={layout.requestTab}
          url={request.url}
        />
        <ApiResponseViewer
          historyItems={history.historyQuery.data ?? []}
          loadingReplay={history.replayHistoryMutation.isPending}
          onReplay={(item: ApiHistoryItem) => {
            layout.setResultTab("response");
            history.replayHistoryMutation.mutate(item.id);
          }}
          onResponseTabChange={layout.setResponseTab}
          onResultTabChange={layout.setResultTab}
          response={request.response}
          responseTab={layout.responseTab}
          resultTab={layout.resultTab}
          sending={request.sendMutation.isPending}
        />
      </SplitPane>
    </form>
  );
}
