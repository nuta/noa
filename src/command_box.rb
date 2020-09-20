require "json"
require "pp"

def pp(obj)
  PP.pp(obj, STDERR)
end

def str_execute(_, code)
  puts _.instance_eval(&code)
rescue Errno::EPIPE
  exit
end

def main(request)
  pp request
  body = request["body"]
  script = request["script"]
  global = request["global"]
  preview = request["preview"]

  message = nil
  num_filtered = 0
  type = nil
  items = []
  if script.empty?
    case body["type"]
    when "files"
      type = "select"
      body["files"].each do |file|
        items << { type: "goto", file: file }
      end
    end
  else

  end
  
  {
    message: message,
    num_filtered: num_filtered,
    body: {
      type: type,
      items: items,
    }
  }
end

request = JSON.parse(STDIN.read)
response = main(request)
puts response.to_json
