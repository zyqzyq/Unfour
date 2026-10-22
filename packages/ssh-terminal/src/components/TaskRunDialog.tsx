import type { SshConnection, SshTaskDetail } from "@unfour/command-client";
import { Lock, LockOpen } from "lucide-react";
import { useId, type ChangeEvent } from "react";
import {
  Badge,
  Button,
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  IconButton,
  SecretInput,
  Select,
  useI18n,
} from "@unfour/ui";
import { detectTaskInputs } from "../model/task-template";

export function TaskRunDialog({
  activeEnvironmentName,
  automaticSecretInputNames,
  connectionId,
  connections,
  environmentId,
  environmentLoadFailed,
  environments,
  error,
  filledFromWorkspace,
  inputValues,
  inputScopeKey,
  onConnectionChange,
  onEnvironmentChange,
  onInputChange,
  onOpenChange,
  onRun,
  onUsePrevious,
  onToggleSecret,
  notice,
  open,
  pending,
  secretInputNames,
  task,
}: {
  activeEnvironmentName: string | null;
  automaticSecretInputNames: string[];
  connectionId: string;
  connections: SshConnection[];
  environmentId: string;
  environmentLoadFailed: boolean;
  environments: Array<{ id: string; name: string }>;
  error: Error | null;
  filledFromWorkspace: boolean;
  inputValues: Record<string, string>;
  inputScopeKey: number;
  onConnectionChange: (connectionId: string) => void;
  onEnvironmentChange: (environmentId: string) => void;
  onInputChange: (name: string, value: string) => void;
  onOpenChange: (open: boolean) => void;
  onRun: () => void;
  onUsePrevious?: () => void;
  onToggleSecret?: (name: string) => void;
  notice?: string | null;
  open: boolean;
  pending: boolean;
  secretInputNames: string[];
  task: SshTaskDetail | null;
}) {
  const { t } = useI18n();
  const detectedInputs = task ? detectTaskInputs(task.steps, true) : [];
  const connection = connections.find((item) => item.id === connectionId && !item.deletedAt && item.workspaceId === task?.task.workspaceId) ?? null;
  const missingEnvironment = Boolean(environmentId && !environments.some((env) => env.id === environmentId));
  const missing = detectedInputs.some((name) => !inputValues[name]?.trim());
  const canRun = Boolean(task && connection && !missing && !missingEnvironment && !environmentLoadFailed && task.steps.some((step) => step.enabled));
  const title = t("ssh.tasks.run.title");
  const secretNames = new Set(secretInputNames);
  const automaticSecretNames = new Set(automaticSecretInputNames);
  const errorMessage = error?.message.includes("SSH_TASK_DEFINITION_CHANGED") ? `${t("ssh.tasks.run.taskChanged")} ${t("ssh.tasks.run.prepareAgain")}`
    : error?.message.includes("SSH_TASK_ENVIRONMENT_CHANGED") ? `${t("ssh.tasks.run.environmentChanged")} ${t("ssh.tasks.run.prepareAgain")}`
    : error?.message.includes("SSH_TASK_ENVIRONMENT_MISSING") ? t("ssh.tasks.run.environmentMissing")
    : error?.message.includes("SSH_TASK_INPUT_SOURCE_MISSING") ? t("ssh.tasks.run.prepareAgain")
    : error?.message.includes("SSH_TASK_INPUTS_CHANGED") ? t("ssh.tasks.run.inputsChanged")
    : error?.message.includes("SSH_TASK_CONNECTION_REQUIRED") ? t("ssh.tasks.run.connectionMissing") : error?.message;

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="w-[min(680px,calc(100vw-32px))]" title={title}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        <DialogBody className="max-h-[70vh] overflow-y-auto">
          <div className="flex flex-col gap-4">
            {onUsePrevious && <div><Button disabled={pending} onClick={onUsePrevious} size="sm" variant="secondary">{t("ssh.tasks.run.usePrevious")}</Button></div>}
            {notice && <p className="text-[12px] text-[var(--u-color-warning)]" role="status">{notice}</p>}
            <div className="grid grid-cols-2 gap-3 text-[12px]">
              <Summary label={t("ssh.tasks.run.taskName")} value={task?.task.name ?? "—"} />
              <Summary label={t("ssh.tasks.run.host")} value={connection ? `${connection.username}@${connection.host}:${connection.port}` : "—"} />
            </div>
            <label className="flex flex-col gap-1">
              <span className="text-[11px] font-medium text-[var(--u-color-text-muted)]">
                {t("ssh.tasks.run.connection")}
              </span>
              <Select disabled={pending} onChange={(event) => onConnectionChange(event.target.value)} value={connectionId}>
                <option value="">{t("ssh.tasks.run.selectConnection")}</option>
                {connectionId && !connection && <option disabled value={connectionId}>{t("ssh.tasks.run.connectionMissing")}</option>}
                {connections.filter((item) => !item.deletedAt && item.workspaceId === task?.task.workspaceId).map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.name} · {item.host}
                  </option>
                ))}
              </Select>
            </label>

            <label className="flex flex-col gap-1">
              <span className="text-[11px] font-medium text-[var(--u-color-text-muted)]">
                {t("ssh.tasks.run.environment")}
              </span>
              <Select
                disabled={pending}
                onChange={(event) => onEnvironmentChange(event.target.value)}
                value={environmentId}
              >
                <option value="">{t("ssh.tasks.run.workspaceOnly")}</option>
                {missingEnvironment && <option disabled value={environmentId}>{t("ssh.tasks.run.environmentMissing")}</option>}
                {environments.map((environment) => (
                  <option key={environment.id} value={environment.id}>
                    {environment.name}
                  </option>
                ))}
              </Select>
            </label>
            {missingEnvironment && <p className="text-[12px] text-[var(--u-color-danger)]" role="alert">{t("ssh.tasks.run.environmentMissing")}</p>}
            {!connection && <p className="text-[12px] text-[var(--u-color-danger)]" role="alert">{t("ssh.tasks.run.connectionMissing")}</p>}
            {environmentLoadFailed && (
              <p className="text-[12px] text-[var(--u-color-warning)]" role="alert">
                {t("ssh.tasks.run.environmentLoadFailed")}
              </p>
            )}

            <section>
              <h3 className="mb-2 text-[12px] font-semibold text-[var(--u-color-text)]">
                {t("ssh.tasks.run.inputs")}
              </h3>
              {filledFromWorkspace && (
                <p className="mb-2 text-[11px] text-[var(--u-color-text-muted)]">
                  {activeEnvironmentName
                    ? t("ssh.tasks.run.workspaceDefaultsWithEnv", {
                        name: activeEnvironmentName,
                      })
                    : t("ssh.tasks.run.workspaceDefaults")}
                </p>
              )}
              {detectedInputs.length ? (
                <div className="grid grid-cols-2 gap-2">
                  {detectedInputs.map((name, index) => (
                    <TaskRunInputField
                      automatic={automaticSecretNames.has(name)}
                      autoFocus={index === 0}
                      inputScopeKey={inputScopeKey}
                      key={name}
                      name={name}
                      onChange={(value) => onInputChange(name, value)}
                      onToggleSecret={onToggleSecret}
                      pending={pending}
                      secret={secretNames.has(name)}
                      value={inputValues[name] ?? ""}
                    />
                  ))}
                </div>
              ) : (
                <p className="text-[12px] text-[var(--u-color-text-muted)]">
                  {t("ssh.tasks.run.noInputs")}
                </p>
              )}
            </section>

            <section>
              <h3 className="mb-2 text-[12px] font-semibold text-[var(--u-color-text)]">
                {t("ssh.tasks.run.steps")}
              </h3>
              <ol className="divide-y divide-[var(--u-color-border)] border border-[var(--u-color-border)]">
                {task?.steps.map((step) => (
                  <li className="flex h-8 items-center gap-2 px-2 text-[12px]" key={step.id}>
                    <span className="w-5 text-right font-mono text-[var(--u-color-text-soft)]">{step.position + 1}</span>
                    <Badge>{t(`ssh.tasks.stepTypes.${step.stepType}`)}</Badge>
                    <span className="min-w-0 flex-1 truncate text-[var(--u-color-text)]">{step.name}</span>
                    {!step.enabled && <span className="text-[var(--u-color-text-soft)]">{t("ssh.tasks.run.disabled")}</span>}
                  </li>
                ))}
              </ol>
            </section>
            {missing && (
              <p className="text-[12px] text-[var(--u-color-danger)]" role="alert">
                {t("ssh.tasks.run.missingInputs")}
              </p>
            )}
            {error && (
              <p className="whitespace-pre-wrap text-[12px] text-[var(--u-color-danger)]" role="alert">
                {errorMessage}
              </p>
            )}
          </div>
        </DialogBody>
        <DialogFooter>
          <DialogClose asChild>
            <Button type="button" variant="secondary">
              {t("ssh.tasks.actions.cancel")}
            </Button>
          </DialogClose>
          <Button disabled={!canRun || pending} onClick={onRun} type="button">
            {pending ? t("ssh.tasks.run.starting") : t("ssh.tasks.actions.run")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function TaskRunInputField({ automatic, autoFocus, inputScopeKey, name, onChange, onToggleSecret, pending, secret, value }: {
  automatic: boolean;
  autoFocus: boolean;
  inputScopeKey: number;
  name: string;
  onChange: (value: string) => void;
  onToggleSecret?: (name: string) => void;
  pending: boolean;
  secret: boolean;
  value: string;
}) {
  const { t } = useI18n();
  const id = useId();
  const sensitive = automatic || secret;
  const action = t(automatic ? "ssh.tasks.run.secretInput" : sensitive ? "ssh.tasks.run.unmarkSensitive" : "ssh.tasks.run.markSensitive");
  const tooltip = automatic ? t("ssh.tasks.run.automaticSensitive") : action;
  const inputProps = { id, "aria-label": name, autoFocus, disabled: pending, value,
    onChange: (event: ChangeEvent<HTMLInputElement>) => onChange(event.target.value) };

  return (
    <div className="flex min-w-0 flex-col gap-1">
      <div className="relative flex min-w-0 items-center gap-1">
        <label className="min-w-0 truncate font-mono text-[11px] text-[var(--u-color-text-muted)]" htmlFor={id} title={name}>{name}</label>
        <IconButton
          aria-description={automatic ? tooltip : undefined}
          aria-disabled={automatic || !onToggleSecret}
          aria-pressed={sensitive}
          className={`static h-5 w-5 ${sensitive ? "text-[var(--u-color-text-muted)]" : "text-[var(--u-color-text-soft)]"} [&>span]:bottom-full [&>span]:left-0 [&>span]:top-auto [&>span]:mb-1 [&>span]:mt-0 [&>span]:w-max [&>span]:max-w-full [&>span]:translate-x-0 [&>span]:whitespace-normal`}
          disabled={pending}
          label={`${name}: ${action}`}
          onClick={() => { if (!automatic) onToggleSecret?.(name); }}
          size="compact"
          tooltip={tooltip}
        >
          {sensitive ? <Lock aria-hidden="true" size={12} /> : <LockOpen aria-hidden="true" size={12} />}
        </IconButton>
      </div>
      {sensitive ? <SecretInput {...inputProps} key={inputScopeKey} /> : <Input {...inputProps} className="min-w-0" type="text" />}
    </div>
  );
}

function Summary({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span className="block text-[11px] text-[var(--u-color-text-muted)]">{label}</span>
      <span className="mt-0.5 block truncate text-[var(--u-color-text)]">{value}</span>
    </div>
  );
}
