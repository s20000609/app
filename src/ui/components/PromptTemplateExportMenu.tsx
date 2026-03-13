import { FileCode, Package } from "lucide-react";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuLabel } from "./BottomMenu";

export type PromptTemplateExportFormat = "external_json" | "usc";

interface PromptTemplateExportMenuProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (format: PromptTemplateExportFormat) => void;
  exporting?: boolean;
}

const FORMATS: Array<{
  id: PromptTemplateExportFormat;
  title: string;
  description: string;
  icon: typeof FileCode;
  color: string;
}> = [
  {
    id: "usc",
    title: "Unified System Card",
    description: "Portable USC export for prompt templates.",
    icon: Package,
    color: "from-emerald-500 to-emerald-600",
  },
  {
    id: "external_json",
    title: "Legacy Prompt JSON",
    description: "Current external prompt pack format.",
    icon: FileCode,
    color: "from-amber-500 to-orange-600",
  },
];

export function PromptTemplateExportMenu({
  isOpen,
  onClose,
  onSelect,
  exporting = false,
}: PromptTemplateExportMenuProps) {
  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title="Export Format">
      <div className="space-y-4">
        <MenuLabel>Select a format</MenuLabel>
        <MenuButtonGroup>
          {FORMATS.map((format) => (
            <MenuButton
              key={format.id}
              icon={<format.icon className="h-4 w-4" />}
              title={format.title}
              description={format.description}
              color={format.color}
              onClick={() => onSelect(format.id)}
              disabled={exporting}
            />
          ))}
        </MenuButtonGroup>
      </div>
    </BottomMenu>
  );
}
