import { useState, type ReactNode } from "react";
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
import type {
  DesktopAppExtensionContext,
  DesktopAppSettingsSection,
} from "../../extensions";

type SettingsSection = "general" | "mcp" | "about";

export function SettingsDialog({
  extensionContext,
  extensionSections = [],
  onOpenChange,
  open,
}: {
  extensionContext: DesktopAppExtensionContext;
  extensionSections?: readonly DesktopAppSettingsSection[];
  onOpenChange: (open: boolean) => void;
  open: boolean;
}) {
  const { t } = useI18n();
  const [activeSection, setActiveSection] = useState<string>("general");
  const coreSections: { id: SettingsSection; label: string }[] = [
    { id: "general", label: t("app.settings.sections.general") },
    { id: "mcp", label: t("app.settings.sections.mcp") },
    { id: "about", label: t("app.settings.sections.about") },
  ];
  const sections: { id: string; label: ReactNode }[] = [
    ...coreSections.slice(0, -1),
    ...extensionSections.map(({ id, label }) => ({ id, label })),
    coreSections[coreSections.length - 1],
  ];
  const activeSectionExists = sections.some((section) => section.id === activeSection);
  const resolvedActiveSection = activeSectionExists ? activeSection : "general";
  const activeExtensionSection = extensionSections.find(
    (section) => section.id === resolvedActiveSection,
  );
  const ExtensionSection = activeExtensionSection?.component;

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="w-[min(900px,calc(100vw-32px))]">
        <DialogHeader>
          <div className="min-w-0">
            <DialogTitle>{t("app.settings.title")}</DialogTitle>
            <DialogDescription>{t("app.settings.description")}</DialogDescription>
          </div>
          <DialogXClose label={t("app.settings.close")} />
        </DialogHeader>
        <DialogBody className="grid h-[600px] grid-cols-[154px_minmax(0,1fr)] overflow-hidden p-0">
          <nav
            aria-label={t("app.settings.navigationLabel")}
            className="border-r border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-2"
          >
            {sections.map((section) => {
              const active = section.id === resolvedActiveSection;
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
            {resolvedActiveSection === "general" && <SettingsGeneral />}
            {resolvedActiveSection === "mcp" && <SettingsMcp />}
            {resolvedActiveSection === "about" && <SettingsAbout />}
            {ExtensionSection && <ExtensionSection {...extensionContext} />}
          </section>
        </DialogBody>
      </DialogContent>
    </Dialog>
  );
}
