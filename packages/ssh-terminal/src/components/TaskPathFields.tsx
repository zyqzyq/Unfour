import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  IconButton,
  Input,
  useI18n,
} from "@unfour/ui";
import { FolderOpen } from "lucide-react";

export function LocalPathField({
  label,
  mode,
  onChange,
  value,
}: {
  label: string;
  mode: "upload" | "download";
  onChange: (value: string) => void;
  value: string;
}) {
  const { t } = useI18n();

  async function pick(kind: "file" | "directory" | "save") {
    try {
      if (kind === "save") {
        const target = await saveFileDialog({
          defaultPath: value.trim() || undefined,
        });
        if (target) onChange(target);
        return;
      }
      const selection = await openFileDialog({
        defaultPath: value.trim() || undefined,
        directory: kind === "directory",
        multiple: false,
      });
      if (typeof selection === "string") onChange(selection);
    } catch {
      // Dialog unavailable in browser mocks, or user cancelled.
    }
  }

  return (
    <label className="flex min-w-0 flex-col gap-1">
      <span className="text-[11px] font-medium text-[var(--u-color-text-muted)]">{label}</span>
      <div className="flex gap-1">
        <Input
          className="min-w-0 flex-1 font-mono text-[12px]"
          onChange={(event) => onChange(event.target.value)}
          placeholder={t("ssh.tasks.editor.localPathPlaceholder")}
          value={value}
        />
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <IconButton label={t("ssh.tasks.editor.browseLocal")} size="compact">
              <FolderOpen size={13} />
            </IconButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            {mode === "upload" ? (
              <>
                <DropdownMenuItem onSelect={() => void pick("file")}>
                  {t("ssh.tasks.editor.browseFile")}
                </DropdownMenuItem>
                <DropdownMenuItem onSelect={() => void pick("directory")}>
                  {t("ssh.tasks.editor.browseFolder")}
                </DropdownMenuItem>
              </>
            ) : (
              <>
                <DropdownMenuItem onSelect={() => void pick("save")}>
                  {t("ssh.tasks.editor.browseSave")}
                </DropdownMenuItem>
                <DropdownMenuItem onSelect={() => void pick("directory")}>
                  {t("ssh.tasks.editor.browseFolder")}
                </DropdownMenuItem>
              </>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <span className="text-[10px] leading-4 text-[var(--u-color-text-soft)]">
        {t("ssh.tasks.editor.localPathHint")}
      </span>
    </label>
  );
}
