'use strict';
const readline = require('node:readline');
const input = readline.createInterface({input: process.stdin, terminal: false});
function send(invocation_id, message) {
    process.stdout.write(JSON.stringify({protocol_version: 2, invocation_id, message}) + '\n');
}
input.on('line', line => {
    const frame = JSON.parse(line);
    const {invocation_id, message} = frame;
    if (message.type === 'invoke') {
        send(invocation_id, {
            type: 'host_call',
            call_id: 'callback',
            method: 'read_item',
            input: {},
            authority: message.authority
        });
    } else if (message.type === 'host_reply') {
        send(invocation_id, {type: 'complete', data: {ok: message.ok}});
    }
});
