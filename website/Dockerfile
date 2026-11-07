FROM nginx:alpine
LABEL maintainer="Nam Tran <support@namtr.dev>"

# Copy static website assets to Nginx default html folder
COPY . /usr/share/nginx/html

EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
