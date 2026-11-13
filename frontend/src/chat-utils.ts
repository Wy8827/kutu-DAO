export const CHAT_MESSAGE_MAX_LENGTH = 1000;

export type ChatMessage = {
  id: string;
  circle_id: string;
  sender_near_account_id: string;
  content: string;
  created_at: string;
  edited_at: string | null;
};

export function validateChatMessage(value: string): string {
  const content = value.trim();
  if (!content) throw new Error('Write a message before sending.');
  if (content.length > CHAT_MESSAGE_MAX_LENGTH) throw new Error(`Messages can be up to ${CHAT_MESSAGE_MAX_LENGTH} characters.`);
  return content;
}

type RealtimeChannelLike = {
  on(event: 'postgres_changes', filter: { event: 'INSERT'; schema: 'public'; table: 'chat_messages'; filter: string }, callback: (payload: { new: ChatMessage }) => void): RealtimeChannelLike;
  subscribe(callback: (status: string) => void): RealtimeChannelLike;
};
type RealtimeClientLike = {
  channel(name: string): RealtimeChannelLike;
  removeChannel(channel: RealtimeChannelLike): Promise<unknown>;
};

export function subscribeToCircleMessages(client: RealtimeClientLike, circleId: string, onInsert: (message: ChatMessage) => void, onStatus: (status: string) => void) {
  const channel = client
    .channel(`circle-chat:${circleId}`)
    .on('postgres_changes', {
      event: 'INSERT',
      schema: 'public',
      table: 'chat_messages',
      filter: `circle_id=eq.${circleId}`,
    }, (payload: { new: ChatMessage }) => onInsert(payload.new))
    .subscribe(onStatus);
  return () => { void client.removeChannel(channel); };
}
