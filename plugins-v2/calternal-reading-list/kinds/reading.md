---
kind: reading
name: Reading
format: 1
folder: Reading
editable: outside
fields:
  title: text, required
  url: url, required
  read: yes/no, default no
  added: date, default today
  site: = domain(url)
row: title, site
search: title, site, body
operations:
  add: url, title
  mark read: set read = yes, when not read, swipe, primary
---
Links saved to read later. One file per link.
