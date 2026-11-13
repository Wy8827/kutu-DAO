import test from 'node:test';
import assert from 'node:assert/strict';
import { CHAT_MESSAGE_MAX_LENGTH, subscribeToCircleMessages, validateChatMessage } from '../src/chat-utils.ts';

test('message validation trims content and rejects empty or overlong input', () => {
  assert.equal(validateChatMessage('  hello  '), 'hello');
  assert.throws(() => validateChatMessage(' \n\t '), /Write a message/);
  assert.equal(validateChatMessage('x'.repeat(CHAT_MESSAGE_MAX_LENGTH)).length, CHAT_MESSAGE_MAX_LENGTH);
  assert.throws(() => validateChatMessage('x'.repeat(CHAT_MESSAGE_MAX_LENGTH + 1)), /1000 characters/);
});

test('realtime subscription is circle scoped and cleanup removes its channel', () => {
  const calls: unknown[] = [];
  const channel = {
    on(event: string, filter: unknown, callback: (payload: unknown) => void) {
      calls.push({ event, filter });
      (this as any).callback = callback;
      return this;
    },
    subscribe(callback: (status: string) => void) { calls.push('subscribed'); callback('SUBSCRIBED'); return this; },
  } as any;
  const client = {
    channel(name: string) { calls.push(name); return channel; },
    removeChannel(value: unknown) { calls.push(value); return Promise.resolve('ok'); },
  };
  const received: unknown[] = [];
  const statuses: string[] = [];
  const cleanup = subscribeToCircleMessages(client, '42', (message) => received.push(message), (status) => statuses.push(status));
  assert.equal(calls[0], 'circle-chat:42');
  assert.deepEqual((calls[1] as any).filter, { event: 'INSERT', schema: 'public', table: 'chat_messages', filter: 'circle_id=eq.42' });
  assert.deepEqual(statuses, ['SUBSCRIBED']);
  const message = { id: 'm1', circle_id: '42' };
  channel.callback({ new: message });
  assert.deepEqual(received, [message]);
  cleanup();
  assert.equal(calls.at(-1), channel);
});
