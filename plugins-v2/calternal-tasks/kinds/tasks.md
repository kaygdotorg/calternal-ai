---
kind: tasks
name: Task
format: 1
folder: Tasks
file: {title}
editable: outside
fields:
  title: text, required, heading
  status: choice(todo|doing|done|cancelled|someday), default todo
  scheduled: when
  due: when
  priority: choice(urgent|high|medium|low)
  tags: tags
  pinned: yes/no, default no
  waiting on: text
  repeat: repeat
  repeat from: choice(schedule|completion), default schedule
  completed: time
  is open: = status in ("todo", "doing")
  first date: = first(due, scheduled)
  overdue: = is open and first date < today()
  next: = next(repeat, after: if(repeat from = "completion", first(completed, scheduled), scheduled))
together:
  repeat, repeat from
row: title, first date, priority
search: title, tags, body
operations:
  add: title, scheduled, due, priority, tags
  change
  done: set status = "done", set completed = now(), when is open, swipe, primary
  won't do: set status = "cancelled", when is open, menu
  schedule: set scheduled = ask, menu
---
One task per note.
