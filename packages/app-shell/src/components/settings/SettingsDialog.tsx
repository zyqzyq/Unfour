import { useState } from "react";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogXClose,
  cn,
  useI18n,
} from "@unfour/ui";
import { SettingsAbout } from "./SettingsAbout";
import { SettingsGeneral } from "./SettingsGeneral";
import { SettingsMcp } from "./SettingsMcp";

type SettingsSection = "general" | "mcp" | "about";

export function SettingsDialog({
  onOpenChange,
  open,
}: {
  onOpenChange: (open: boolean) => void;
  open: boolean;
}) {
  const { t } = useI18n();
  const [activeSection, setActiveSection] = useState<SettingsSection>("general");
  const sections: { id: SettingsSection; label: string }[] = [
    { id: "general", label: t("app.settings.sections.general") },
    { id: "mcp", label: t("app.settings.sections.mcp") },
    { id: "about", label: t("app.settings.sections.about") },
  ];

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="w-[min(760px,calc(100vw-32px))]">
        <DialogHeader>
          <div className="min-w-0">
            <DialogTitle>{t("app.settings.title")}</DialogTitle>
            <DialogDescription>{t("app.settings.description")}</DialogDescription>
          </div>
          <DialogXClose label={t("app.settings.close")} />
        </DialogHeader>
        <DialogBody className="grid min-h-[420px] grid-cols-[154px_minmax(0,1fr)] overflow-hidden p-0">
          <nav
            aria-label={t("app.settings.navigationLabel")}
            className="border-r border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-2"
          >
            {sections.map((section) => {
              const active = section.id === activeSection;
              return (
                <Button
                  aria-current={active ? "page" : undefined}
                  className={cn(
                    "mb-1 w-full justify-start px-2",
                    active &&
                      "border-[var(--u-color-primary-soft)] bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)] hover:bg-[var(--u-color-primary-soft)]",
                  )}
                  key={section.id}
                  onClick={() => setActiveSection(section.id)}
                  size="sm"
                  type="button"
                  variant="ghost"
                >
                  {section.label}
                </Button>
              );
            })}
          </nav>
          <section className="min-w-0 overflow-y-auto p-4">
            {activeSection === "general" && <SettingsGeneral />}
            {activeSection === "mcp" && <SettingsMcp />}
            {activeSection === "about" && <SettingsAbout />}
          </section>
        </DialogBody>
      </DialogContent>
    </Dialog>
  );
}
