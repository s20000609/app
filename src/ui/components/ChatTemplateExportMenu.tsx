import { FileCode, Package } from "lucide-react";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuLabel } from "./BottomMenu";

export type ChatTemplateExportFormat = "json" | "usc";

interface ChatTemplateExportMenuProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (format: ChatTemplateExportFormat) => void;
  exporting?: boolean;
}

const FORMATS: Array<{
  id: ChatTemplateExportFormat;
  title: string;
  description: string;
  icon: typeof FileCode;
  color: string;
}> = [
  {
    id: "usc",
    title: "Unified System Card",
    description: "Portable USC export for chat templates.",
    icon: Package,
    color: "from-emerald-500 to-emerald-600",
  },
  {
    id: "json",
    title: "Chat Template JSON",
    description: "Native chat template export format.",
    icon: FileCode,
    color: "from-amber-500 to-orange-600",
  },
];

export function ChatTemplateExportMenu({
  isOpen,
  onClose,
  onSelect,
  exporting = false,
}: ChatTemplateExportMenuProps) {
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
