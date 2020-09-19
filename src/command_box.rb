require "json"

def str_execute(text, code)
  puts text.instance_eval(&code)
rescue Errno::EPIPE
  exit
end

request = JSON.parse(STDIN.read)
STDERR.puts request
raise
