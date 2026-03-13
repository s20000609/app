import { FileCode, Package } from "lucide-react";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuLabel } from "./BottomMenu";

export type LorebookExportFormat = "legacy_json" | "usc";

interface LorebookExportMenuProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (format: LorebookExportFormat) => void;
  exporting?: boolean;
}

const FORMATS: Array<{
  id: LorebookExportFormat;
  title: string;
  description: string;
  icon: typeof FileCode;
  color: string;
}> = [
  {
    id: "usc",
    title: "Unified System Card",
    description: "Portable USC export for lorebooks.",
    icon: Package,
    color: "from-emerald-500 to-emerald-600",
  },
  {
    id: "legacy_json",
    title: "Lorebook JSON",
    description: "Current lorebook export format.",
    icon: FileCode,
    color: "from-amber-500 to-orange-600",
  },
];

export function LorebookExportMenu({
  isOpen,
  onClose,
  onSelect,
  exporting = false,
}: LorebookExportMenuProps) {
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
