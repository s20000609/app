import { MessageSquare, FileText, Star } from "lucide-react";
import { BottomMenu, MenuButton, MenuButtonGroup } from "../../../components/BottomMenu";
import type { ChatTemplate } from "../../../../core/storage/schemas";

interface ChatTemplateSelectorProps {
  isOpen: boolean;
  onClose: () => void;
  templates: ChatTemplate[];
  defaultTemplateId?: string | null;
  onSelect: (templateId: string | null) => void;
}

export function ChatTemplateSelector({
  isOpen,
  onClose,
  templates,
  defaultTemplateId,
  onSelect,
}: ChatTemplateSelectorProps) {
  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title="Start with a template?">
      <MenuButtonGroup>
        <MenuButton
          icon={<FileText className="h-4 w-4" />}
          title="No template"
          description="Start with scene only"
          color="from-blue-500 to-cyan-600"
          onClick={() => onSelect(null)}
        />
        {templates.map((template) => {
          const preview = template.messages
            .slice(0, 2)
            .map(
              (m) =>
                `${m.role === "user" ? "You" : "Bot"}: ${m.content.slice(0, 60)}${m.content.length > 60 ? "..." : ""}`,
            )
            .join(" | ");
          return (
            <MenuButton
              key={template.id}
              icon={<MessageSquare className="h-4 w-4" />}
              title={template.name}
              description={
                preview ||
                `${template.messages.length} message${template.messages.length !== 1 ? "s" : ""}`
              }
              color="from-blue-500 to-cyan-600"
              rightElement={
                defaultTemplateId === template.id ? (
                  <Star className="h-3.5 w-3.5 fill-warning text-warning" />
                ) : undefined
              }
              onClick={() => onSelect(template.id)}
            />
          );
        })}
      </MenuButtonGroup>
    </BottomMenu>
  );
}
