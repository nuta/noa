require "json"

def str_execute(text, code)
  puts text.instance_eval(&code)
rescue Errno::EPIPE
  exit
end

request = JSON.parse(STDIN.read)
global = request["global"]
preview = request["preview"]

message = "#{Time.now}"
num_filtered = 0
body = {
  type: "select",
  items: [
    { type: "file", display_name: "foo", path: "bar" }
  ],
}

response = {
  message: message,
  num_filtered: num_filtered,
  body: body,
}

puts response.to_json
